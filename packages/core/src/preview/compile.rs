//! Сборка документа (contracts/compile-engine.md).
//!
//! Две фазы. Синхронная — [`compile_with`]: работает над готовым снимком и к
//! файловой системе не обращается, поэтому одинакова на обоих таргетах.
//! Асинхронная — [`compile_note_resolved`]: чередует сборку с догрузкой
//! связанных файлов через `VaultBackend`, пока компилятор не перестанет
//! просить недостающее (T014-T015, research.md §2).
//!
//! Чего здесь пока нет: предела времени сборки в 10 секунд (T023, FR-026).

use serde::{Deserialize, Serialize};
use typst::foundations::Bytes;
use typst::syntax::package::{PackageSpec, PackageVersion};
use typst::syntax::FileId;
use typst_layout::PagedDocument;

use super::cache::PackageCache;
use super::diagnostics::{sort_by_location, translate, Diagnostic, DiagnosticSeverity};
use super::fonts::FontSet;
use super::pages::{render_pages, RenderedPage};
use super::packages::{fetch_and_install, InstallError, InstallReport, TemplateSource};
use super::resolve::{read_dependency, vault_path, ResolveError};
use super::snapshot::{DependencySnapshot, SnapshotError};
use super::world::PreviewWorld;
use crate::vault::backend::VaultBackend;

/// Чем закончилась сборка (data-model.md «CompileOutcome»).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum CompileOutcome {
    /// Документ собран. Диагностики при этом могут содержать предупреждения,
    /// но **ни одной** ошибки (запрещённое состояние из data-model.md).
    Succeeded,
    /// Документ не собран; среди диагностик есть хотя бы одна ошибка.
    Failed,
}

/// Свёрстанный вид (data-model.md «CompiledDocument»).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CompiledDocument {
    /// Версия текста, из которой собран результат. Результат с версией меньше
    /// уже показанной UI обязан отбросить (FR-009).
    pub revision: u64,
    pub note_path: String,
    pub outcome: CompileOutcome,
    /// **Все** страницы документа по порядку, каждая со своим `digest`. Ядро
    /// не фильтрует: пропуск неизменившихся — забота UI (research.md §5).
    pub pages: Vec<RenderedPage>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Собирает заметку из текста редактора.
///
/// Текст приходит из буфера, а не с диска: иначе требование «без кнопки
/// сохранить» невыполнимо (FR-002, FR-006). Файл vault при этом не читается и
/// не изменяется.
pub fn compile_note(
    note_path: &str,
    source_text: &str,
    revision: u64,
    fonts: FontSet,
) -> Result<CompiledDocument, SnapshotError> {
    let main = DependencySnapshot::file_id(note_path)?;
    let mut snapshot = DependencySnapshot::new();
    snapshot.insert(main, Bytes::new(source_text.as_bytes().to_vec()));
    Ok(compile_with(main, note_path, snapshot, revision, fonts).0)
}

/// Собирает заметку, догружая связанные файлы через vault (T014-T015).
///
/// Чередование обязательно: какие файлы нужны документу, известно только из
/// самой сборки — компилятор просит их по ходу, и до первого прохода список
/// зависимостей не существует (research.md §2). Поэтому сборка повторяется,
/// пока не перестанут появляться новые промахи или пока не кончится предел в
/// [`MAX_RESOLVE_ITERATIONS`] проходов.
///
/// Чтение файлов — через `VaultBackend`, своего доступа к диску здесь нет
/// (принцип I, contracts/compile-engine.md § Фаза резолва).
/// `packages` — файлы шаблонов, пережившие прошлые сборки. Без него каждая
/// сборка заново поднимала бы шаблон по одному файлу за проход, тратя на это
/// полную пересборку документа (`cache.rs` объясняет цену). Разовому вызову —
/// тесту, проверке — годится пустой `PackageCache::new()`.
pub async fn compile_note_resolved<B: VaultBackend>(
    backend: &B,
    packages: &PackageCache,
    note_path: &str,
    source_text: &str,
    revision: u64,
    fonts: FontSet,
) -> Result<CompiledDocument, SnapshotError> {
    let (document, failures) =
        compile_collecting_failures(backend, packages, note_path, source_text, revision, fonts)
            .await?;
    Ok(with_resolve_failures(document, failures))
}

/// То же, но причины отказов возвращаются **структурно**, а не превращёнными в
/// текст диагностик.
///
/// Нужно загрузке шаблонов (фича 007): чтобы понять, какой пакет недостаёт, ей
/// нужен `ResolveError::PackageMissing`, а не строка сообщения. Разбирать
/// собственный текст обратно — верный способ разойтись с ним при первой же
/// правке формулировки.
pub(crate) async fn compile_collecting_failures<B: VaultBackend>(
    backend: &B,
    packages: &PackageCache,
    note_path: &str,
    source_text: &str,
    revision: u64,
    fonts: FontSet,
) -> Result<(CompiledDocument, Vec<(String, ResolveError)>), SnapshotError> {
    let main = DependencySnapshot::file_id(note_path)?;
    let mut snapshot = DependencySnapshot::new();
    snapshot.insert(main, Bytes::new(source_text.as_bytes().to_vec()));
    // Файлы шаблонов, известные с прошлых сборок, — сразу в снимок. Каждый
    // промах здесь стоил бы отдельного прохода, то есть полной пересборки.
    packages.seed(&mut snapshot);

    // Причины отказов копятся между проходами: файл, который не прочитался,
    // на следующем проходе уже не запрашивается, а его имя обязано дойти до
    // пользователя названным (FR-021).
    let mut failures: Vec<(String, ResolveError)> = Vec::new();

    loop {
        let (document, mut returned) =
            compile_with(main, note_path, snapshot, revision, fonts.clone());

        let missing = returned.missing();
        if missing.is_empty() || !returned.begin_iteration() {
            packages.absorb(&returned);
            return Ok((document, failures));
        }

        // Проход считается продуктивным, если хоть один файл догрузился. Без
        // этой проверки недостающий файл крутил бы цикл до конца предела и
        // тратил восемь сборок на заведомо тот же отказ.
        let mut progressed = false;
        for id in missing {
            match read_dependency(backend, id).await {
                Ok(bytes) => {
                    returned.insert(id, bytes);
                    progressed = true;
                }
                Err(error) => {
                    let name = vault_path(id).unwrap_or_else(|| "<пакет>".to_owned());
                    // Тот же файл мог промахнуться дважды — имя в списке одно.
                    if !failures.iter().any(|(known, _)| known == &name) {
                        failures.push((name, error));
                    }
                    // Снимаем с учёта недостающих: повторять чтение незачем,
                    // ответ не изменится в пределах одной сборки.
                    returned.forget_missing(id);
                }
            }
        }

        if !progressed {
            let (document, returned) = compile_with(main, note_path, returned, revision, fonts);
            packages.absorb(&returned);
            return Ok((document, failures));
        }

        snapshot = returned;
    }
}

/// Собирает заметку, доустанавливая недостающие внешние шаблоны (фича 007,
/// FR-001).
///
/// # Почему две сборки, а не одна
///
/// Какие шаблоны нужны документу, известно только из самой сборки: подключение
/// может прийти из файла, который сам подгружен по ходу. Поэтому порядок такой:
/// собрать как есть → посмотреть, каких **шаблонов** не хватило → доустановить
/// → собрать снова.
///
/// Вторая сборка происходит **только** если чего-то действительно не хватило и
/// хоть один шаблон удалось поставить. Заметка без подключений и заметка со
/// всеми установленными шаблонами проходят ровно один раз и в сеть не заходят
/// (FR-002, FR-008, SC-002, SC-009).
///
/// Отсюда же следует, что `compile_note_resolved` остаётся неизменной и
/// по-прежнему принимает `&B`: сеть и запись появляются только на этом,
/// отдельном пути.
pub async fn compile_note_with_templates<B: VaultBackend>(
    backend: &mut B,
    packages: &PackageCache,
    note_path: &str,
    source_text: &str,
    revision: u64,
    fonts: FontSet,
    policy: &TemplatePolicy,
) -> Result<CompiledDocument, SnapshotError> {
    let (document, failures) = compile_collecting_failures(
        backend,
        packages,
        note_path,
        source_text,
        revision,
        fonts.clone(),
    )
    .await?;

    let missing = missing_templates(&failures);
    if missing.is_empty() {
        return Ok(with_resolve_failures(document, failures));
    }

    let mut notes: Vec<Diagnostic> = Vec::new();
    let mut installed_any = false;
    for spec in missing {
        match install_missing(backend, &spec, policy).await {
            Ok(report) => {
                installed_any = true;
                // Ту же версию могли поставить поверх: прежние байты этого
                // шаблона из кэша уходят, чтобы пересборка увидела диск.
                if let Some(parsed) = parse_spec(&spec) {
                    packages.forget(&parsed);
                }
                // FR-006: молчаливой загрузки не бывает — что и откуда.
                notes.push(Diagnostic {
                    severity: DiagnosticSeverity::Warning,
                    message: format!(
                        "шаблон {} загружен с {} ({} файлов, отпечаток {})",
                        report.spec,
                        report.url,
                        report.file_count,
                        &report.fingerprint[..12.min(report.fingerprint.len())]
                    ),
                    location: None,
                });
            }
            Err(error) => {
                // FR-007: причина и тот адрес, к которому обращались.
                notes.push(Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    message: error.to_string(),
                    location: None,
                });
            }
        }
    }

    if !installed_any {
        let mut document = with_resolve_failures(document, failures);
        document.diagnostics.extend(notes);
        sort_by_location(&mut document.diagnostics);
        return Ok(document);
    }

    // Пересборка: теперь недостающие шаблоны на месте. Прежние причины отказов
    // не переносятся — они относились к состоянию до установки.
    let (document, failures) =
        compile_collecting_failures(backend, packages, note_path, source_text, revision, fonts)
            .await?;
    let mut document = with_resolve_failures(document, failures);
    document.diagnostics.extend(notes);
    sort_by_location(&mut document.diagnostics);
    Ok(document)
}

/// Что делать с недостающими шаблонами (FR-004, FR-009).
#[derive(Debug, Clone, Default)]
pub struct TemplatePolicy {
    /// Ходить ли в сеть самостоятельно. `false` — поведение до фичи 007:
    /// шаблон назван отсутствующим, обращений в сеть нет.
    pub auto_download: bool,
    /// Адреса, указанные пользователем для конкретных шаблонов. Ключ — та же
    /// строка спецификации, что видит пользователь: `@preview/codly:1.3.0`.
    pub sources: Vec<(String, String)>,
}

impl TemplatePolicy {
    /// Источник для шаблона: указанный пользователем адрес либо общий реестр.
    fn source_for(&self, spec: &str) -> TemplateSource {
        self.sources
            .iter()
            .find(|(known, _)| known == spec)
            .map(|(_, url)| TemplateSource::Url { url: url.clone() })
            .unwrap_or(TemplateSource::Registry)
    }
}

/// Спецификации шаблонов, которых не хватило, — из структурных причин отказа.
///
/// Именно из них, а не разбором текста диагностик: собственное сообщение,
/// прочитанное обратно, разошлось бы с собой при первой правке формулировки.
fn missing_templates(failures: &[(String, ResolveError)]) -> Vec<String> {
    let mut specs: Vec<String> = Vec::new();
    for (_, error) in failures {
        if let ResolveError::PackageMissing { spec } = error {
            if !specs.contains(spec) {
                specs.push(spec.clone());
            }
        }
    }
    specs
}

async fn install_missing<B: VaultBackend>(
    backend: &mut B,
    spec_text: &str,
    policy: &TemplatePolicy,
) -> Result<InstallReport, InstallError> {
    if !policy.auto_download {
        return Err(InstallError::AutoDownloadDisabled {
            spec: spec_text.to_string(),
        });
    }
    let spec = parse_spec(spec_text).ok_or_else(|| InstallError::VersionNotFound {
        spec: spec_text.to_string(),
    })?;
    fetch_and_install(backend, &spec, &policy.source_for(spec_text)).await
}

/// Разбирает `@namespace/name:major.minor.patch` обратно в спецификацию.
///
/// Нужен потому, что причина отказа несёт спецификацию **строкой** — ровно той,
/// что видит пользователь. Хранить в ней разобранную структуру значило бы
/// протащить тип компилятора через границу WASM ради одного места.
fn parse_spec(text: &str) -> Option<PackageSpec> {
    let rest = text.strip_prefix('@')?;
    let (namespace, rest) = rest.split_once('/')?;
    let (name, version) = rest.split_once(':')?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(PackageSpec {
        namespace: namespace.into(),
        name: name.into(),
        version: PackageVersion {
            major,
            minor,
            patch,
        },
    })
}

/// Один проход сборки. Возвращает результат и снимок обратно — в нём
/// накопленные файлы и список того, чего не хватило.
fn compile_with(
    main: FileId,
    note_path: &str,
    snapshot: DependencySnapshot,
    revision: u64,
    fonts: FontSet,
) -> (CompiledDocument, DependencySnapshot) {
    let world = PreviewWorld::new(main, snapshot, fonts.into_fonts());
    let compiled = typst::compile::<PagedDocument>(&world);

    let mut diagnostics: Vec<Diagnostic> = compiled
        .warnings
        .iter()
        .map(|d| translate(&world, main, d))
        .collect();

    let (outcome, pages) = match compiled.output {
        Ok(document) => (CompileOutcome::Succeeded, render_pages(&document)),
        Err(errors) => {
            diagnostics.extend(errors.iter().map(|d| translate(&world, main, d)));
            (CompileOutcome::Failed, Vec::new())
        }
    };

    sort_by_location(&mut diagnostics);

    (
        CompiledDocument {
            revision,
            note_path: note_path.to_owned(),
            outcome,
            pages,
            diagnostics,
        },
        world.into_snapshot(),
    )
}

/// Дописывает к результату причины, по которым связанные файлы не прочитались.
///
/// Отдельными диагностиками, а не общей ошибкой: FR-021 требует назвать
/// **каждый** файл и **его** причину.
fn with_resolve_failures(
    mut document: CompiledDocument,
    failures: Vec<(String, ResolveError)>,
) -> CompiledDocument {
    if failures.is_empty() {
        return document;
    }
    for (path, error) in failures {
        document.diagnostics.push(Diagnostic {
            severity: DiagnosticSeverity::Error,
            // `describe_owned`, а не `describe`: у отсутствующего шаблона имя
            // несёт сама причина, и подставленный слева путь внутри пакета
            // пользователю ничего не говорит — он писал в заметке `@preview/…`
            // (фича 006, FR-037).
            message: match &error {
                ResolveError::PackageMissing { .. } => error.describe_owned(),
                _ => format!("{path}: {}", error.describe()),
            },
            // Места в тексте у такой ошибки нет: она про файл целиком, а не
            // про позицию в нём. Перехода к ней быть не должно (FR-033).
            location: None,
        });
    }
    document.outcome = CompileOutcome::Failed;
    document
}
