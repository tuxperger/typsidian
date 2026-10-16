//! Исполнитель сборки в Web Worker (FR-007, FR-026, T022; research.md §3).
//!
//! В воркере живёт **второй экземпляр** этого же wasm-модуля. Основной
//! экземпляр ведёт резолв — читает связанные файлы через vault — и на каждый
//! проход компилятора отправляет воркеру готовый снимок; воркер выполняет
//! синхронную фазу ([`CompileJob::run_inline`]) и возвращает документ и список
//! того, чего не хватило. Поток интерфейса при этом свободен, а сборку,
//! не уложившуюся в предел, обрывает настоящая отмена — `worker.terminate()`.
//!
//! Сам воркер создаёт и убивает интерфейс: это API браузера. Здесь — форма
//! сообщений и обе их стороны, чтобы они не могли разойтись.

use serde::{Deserialize, Serialize};
use typst::foundations::Bytes;
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::compile::{
    CompileExecutor, CompileJob, CompileOutput, CompiledDocument, ExecutorError, PdfMeta, Render,
    BUILD_TIME_LIMIT_SECS,
};
use super::fonts::FontSet;
use super::packages::parse_spec;
use super::snapshot::DependencySnapshot;

/// Предел времени сборки для стороны, которая обрывает воркер. Число задаёт
/// ядро, а не интерфейс (принцип I).
#[wasm_bindgen(js_name = buildTimeLimitMs)]
pub fn build_time_limit_ms() -> u32 {
    BUILD_TIME_LIMIT_SECS * 1000
}

/// `FileId` в виде, который переживает сообщение между экземплярами модуля:
/// сам идентификатор — номер во внутренней таблице экземпляра.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileKey {
    /// Спецификация шаблона (`@preview/имя:1.0.0`); `None` — файл vault.
    package: Option<String>,
    path: String,
}

impl FileKey {
    fn of(id: FileId) -> Self {
        Self {
            package: match id.root() {
                VirtualRoot::Project => None,
                VirtualRoot::Package(spec) => Some(spec.to_string()),
            },
            path: id.vpath().get_without_slash().to_owned(),
        }
    }

    fn id(&self) -> Option<FileId> {
        let root = match &self.package {
            None => VirtualRoot::Project,
            Some(spec) => VirtualRoot::Package(parse_spec(spec)?),
        };
        let path = VirtualPath::new(&self.path).ok()?;
        Some(RootedPath::new(root, path).intern())
    }
}

/// Всё в сообщении, кроме содержимого файлов: оно едет рядом, `Uint8Array`,
/// без перекодирования.
#[derive(Debug, Serialize, Deserialize)]
struct JobHeader {
    main: FileKey,
    note_path: String,
    revision: u64,
    files: Vec<FileKey>,
    /// Идентификатор PDF; `None` — нужны страницы.
    pdf_ident: Option<String>,
    /// Входы блоков выборки (фича 020): JSON-строка для `sys.inputs`.
    #[serde(default)]
    inputs: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ResultHeader {
    document: CompiledDocument,
    missing: Vec<FileKey>,
    /// Файлы, которые компилятор получил из снимка: по ним основной экземпляр
    /// решает, что держать в кэше связанных файлов.
    accessed: Vec<FileKey>,
}

fn field(object: &JsValue, name: &str) -> Result<JsValue, JsValue> {
    js_sys::Reflect::get(object, &JsValue::from_str(name))
}

fn set(object: &js_sys::Object, name: &str, value: &JsValue) {
    // Запись в только что созданный простой объект отказать не может.
    let _ = js_sys::Reflect::set(object, &JsValue::from_str(name), value);
}

fn bytes_array<'a>(items: impl Iterator<Item = &'a [u8]>) -> js_sys::Array {
    items
        .map(|bytes| JsValue::from(js_sys::Uint8Array::from(bytes)))
        .collect()
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(JsValue::from)
}

/// Сообщение воркеру: `{ header, files, fonts }`.
fn encode_job(job: &CompileJob, builtin_fonts: usize) -> Result<JsValue, JsValue> {
    let ids = job.snapshot.dependencies();
    let header = JobHeader {
        main: FileKey::of(job.main),
        note_path: job.note_path.clone(),
        revision: job.revision,
        files: ids.iter().map(|id| FileKey::of(*id)).collect(),
        pdf_ident: match &job.render {
            Render::Pages => None,
            Render::Pdf(meta) => Some(meta.ident.clone()),
        },
        inputs: job.inputs.clone(),
    };
    let files = bytes_array(
        ids.iter()
            .filter_map(|id| job.snapshot.get(*id))
            .map(|bytes| bytes.as_slice()),
    );
    // Встроенный набор у воркера свой; едут только шрифты из vault.
    let extra = job.fonts.data_after(builtin_fonts);
    let fonts = bytes_array(extra.iter().map(|bytes| bytes.as_slice()));

    let message = js_sys::Object::new();
    set(&message, "header", &to_js(&header)?);
    set(&message, "files", &files);
    set(&message, "fonts", &fonts);
    Ok(message.into())
}

thread_local! {
    /// Встроенные шрифты экземпляра-воркера: разбираются один раз, а не на
    /// каждую сборку.
    static BUILTIN: FontSet = FontSet::builtin();
}

/// Сторона воркера: выполняет один проход компилятора.
///
/// Возвращает `{ header, pdf }`. Зовётся из `ui/src/preview/compile-worker.ts`
/// и больше ниоткуда.
#[wasm_bindgen(js_name = runCompileJob)]
pub fn run_compile_job(message: JsValue) -> Result<JsValue, JsValue> {
    let header: JobHeader = serde_wasm_bindgen::from_value(field(&message, "header")?)?;
    let files = js_sys::Array::from(&field(&message, "files")?);
    let fonts = js_sys::Array::from(&field(&message, "fonts")?);
    let bad = |what: &str| JsValue::from_str(&format!("compile job: {what}"));

    let mut snapshot = DependencySnapshot::new();
    for (key, bytes) in header.files.iter().zip(files.iter()) {
        let id = key.id().ok_or_else(|| bad("unreadable file key"))?;
        snapshot.insert(id, Bytes::new(js_sys::Uint8Array::new(&bytes).to_vec()));
    }
    let main = header.main.id().ok_or_else(|| bad("unreadable main key"))?;
    let fonts = BUILTIN.with(FontSet::clone).with_vault_fonts(
        fonts
            .iter()
            .map(|bytes| Bytes::new(js_sys::Uint8Array::new(&bytes).to_vec())),
    );
    let render = match header.pdf_ident {
        None => Render::Pages,
        Some(ident) => Render::Pdf(PdfMeta { ident }),
    };

    let output = CompileJob {
        main,
        note_path: header.note_path,
        snapshot,
        revision: header.revision,
        fonts,
        render,
        inputs: header.inputs,
    }
    .run_inline();

    let reply = js_sys::Object::new();
    let header = ResultHeader {
        missing: output
            .snapshot
            .missing()
            .into_iter()
            .map(FileKey::of)
            .collect(),
        accessed: output
            .snapshot
            .accessed()
            .into_iter()
            .map(FileKey::of)
            .collect(),
        document: output.document,
    };
    set(&reply, "header", &to_js(&header)?);
    if let Some(pdf) = output.pdf {
        set(&reply, "pdf", &js_sys::Uint8Array::from(pdf.as_slice()));
    }
    Ok(reply.into())
}

/// Сторона основного экземпляра: отдаёт проход воркеру.
///
/// `post` — функция интерфейса `(message) => Promise<reply>`: она владеет
/// воркером, ждёт ответа не дольше [`build_time_limit_ms`] и по истечении
/// убивает воркер, отклоняя промис объектом `{ kind: "TimedOut" }`; при отмене
/// сборки делает то же с `{ kind: "Cancelled" }`.
pub struct WorkerExecutor {
    post: js_sys::Function,
    builtin_fonts: usize,
}

impl WorkerExecutor {
    pub fn new(post: js_sys::Function, builtin_fonts: usize) -> Self {
        Self {
            post,
            builtin_fonts,
        }
    }

    async fn ask(&self, job: &CompileJob) -> Result<(ResultHeader, Option<Vec<u8>>), JsValue> {
        let message = encode_job(job, self.builtin_fonts)?;
        let promise = self.post.call1(&JsValue::NULL, &message)?;
        let reply = JsFuture::from(js_sys::Promise::resolve(&promise)).await?;
        let header: ResultHeader = serde_wasm_bindgen::from_value(field(&reply, "header")?)?;
        let pdf = field(&reply, "pdf")?;
        let pdf = (!pdf.is_undefined()).then(|| js_sys::Uint8Array::new(&pdf).to_vec());
        Ok((header, pdf))
    }
}

/// `console.warn` без отдельной возможности `web-sys`: место одно, а
/// глобальный объект есть и в окне, и в воркере.
fn warn(text: &str, detail: &JsValue) {
    let console = field(&js_sys::global(), "console");
    let warn = console
        .as_ref()
        .ok()
        .and_then(|console| field(console, "warn").ok());
    if let (Ok(console), Some(warn)) = (&console, warn) {
        if let Ok(warn) = warn.dyn_into::<js_sys::Function>() {
            let _ = warn.call2(console, &JsValue::from_str(text), detail);
        }
    }
}

/// Отказ, которым интерфейс сообщает, что сам остановил сборку.
fn stop_reason(error: &JsValue) -> Option<ExecutorError> {
    let kind = field(error, "kind").ok()?.as_string()?;
    match kind.as_str() {
        "TimedOut" => Some(ExecutorError::TimedOut),
        "Cancelled" => Some(ExecutorError::Cancelled),
        _ => None,
    }
}

impl CompileExecutor for WorkerExecutor {
    async fn run(&self, job: CompileJob) -> Result<CompileOutput, ExecutorError> {
        match self.ask(&job).await {
            Ok((header, pdf)) => {
                // Снимок остаётся у этого экземпляра; воркер сообщает только,
                // чего компилятору не хватило.
                let snapshot = job.snapshot;
                for id in header.missing.iter().filter_map(FileKey::id) {
                    snapshot.record_missing(id);
                }
                for id in header.accessed.iter().filter_map(FileKey::id) {
                    snapshot.record_access(id);
                }
                Ok(CompileOutput {
                    document: header.document,
                    pdf,
                    snapshot,
                })
            }
            Err(error) if stop_reason(&error).is_some() => {
                Err(stop_reason(&error).unwrap_or(ExecutorError::Cancelled))
            }
            // Воркер не поднялся или ответил не тем: предпросмотр важнее, чем
            // место, где он собран. Причина — в консоль, сборка — здесь.
            Err(error) => {
                warn("compile worker unavailable, building inline:", &error);
                Ok(job.run_inline())
            }
        }
    }
}
