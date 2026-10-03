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
use typst::syntax::FileId;
use typst_layout::PagedDocument;

use super::cache::PackageCache;
use super::diagnostics::{sort_by_location, translate, Diagnostic, DiagnosticSeverity};
use super::fonts::FontSet;
use super::packages::{
    fetch_and_install, source_url, verify_installed, FingerprintCheck, InstallError, InstallReport,
    TemplateSource,
};
use super::pages::{render_pages, RenderedPage};
use super::resolve::{read_dependency, vault_path, ResolveError};
use super::snapshot::{DependencySnapshot, SnapshotError};
use super::world::PreviewWorld;
use crate::messages::{AppMessage, FingerprintActual};
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
    Ok(compile_with(main, note_path, snapshot, revision, fonts, &Render::Pages).0)
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
    let (document, _, failures) = compile_collecting_failures(
        backend,
        packages,
        note_path,
        source_text,
        revision,
        fonts,
        &Render::Pages,
    )
    .await?;
    Ok(with_resolve_failures(document, failures))
}

/// Что получить из свёрстанного документа (009/research.md §3).
///
/// Цикл резолва один на предпросмотр и экспорт: второй его экземпляр был бы
/// вторым мнением о том, как собирается заметка, и разошёлся бы с первым при
/// первой же правке. Различается только последний шаг — во что превратить
/// `PagedDocument`.
#[derive(Debug, Clone)]
pub(crate) enum Render {
    /// Страницы SVG с текстовым слоем — для предпросмотра.
    Pages,
    /// Файл PDF — для экспорта (фича 009).
    Pdf(PdfMeta),
}

/// Метаданные PDF, которые задаёт приложение, а не документ (009/research.md §2).
#[derive(Debug, Clone)]
pub(crate) struct PdfMeta {
    /// Стабильный идентификатор документа между экспортами одной заметки —
    /// путь заметки в vault.
    pub ident: String,
}

/// Байты PDF последнего прохода. `None` — документ не собран или вывод был
/// страницами.
pub(crate) type PdfBytes = Option<Vec<u8>>;

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
    render: &Render,
) -> Result<(CompiledDocument, PdfBytes, Vec<(String, ResolveError)>), SnapshotError> {
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

    // Шрифты из папки vault (FR-027): список — на каждую сборку, чтобы шрифт,
    // положенный в папку, действовал без переоткрытия заметки (FR-029).
    let (vault_fonts, font_problems) = packages.vault_fonts().load(backend).await;
    let fonts = fonts.with_fonts(vault_fonts);
    let with_font_problems = |(mut document, pdf): (CompiledDocument, PdfBytes)| {
        // Предупреждение, а не ошибка: без этого файла документ всё равно
        // собирается — с подстановкой шрифта, о которой скажет сам Typst.
        document.diagnostics.extend(
            font_problems
                .iter()
                .map(|problem| Diagnostic::app(DiagnosticSeverity::Warning, problem.app_message())),
        );
        (document, pdf)
    };

    loop {
        let (document, pdf, mut returned) =
            compile_with(main, note_path, snapshot, revision, fonts.clone(), render);

        let missing = returned.missing();
        if missing.is_empty() || !returned.begin_iteration() {
            packages.absorb(&returned);
            let (document, pdf) = with_font_problems((document, pdf));
            return Ok((document, pdf, failures));
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
            let (document, pdf, returned) =
                compile_with(main, note_path, returned, revision, fonts, render);
            packages.absorb(&returned);
            let (document, pdf) = with_font_problems((document, pdf));
            return Ok((document, pdf, failures));
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
    let (document, _) = build_with_templates(
        backend,
        packages,
        note_path,
        source_text,
        revision,
        fonts,
        policy,
        &Render::Pages,
    )
    .await?;
    Ok(document)
}

/// Тело [`compile_note_with_templates`] с выбором вывода: тот же путь
/// доустановки шаблонов обслуживает и экспорт в PDF (фича 009), иначе заметка
/// с внешним шаблоном собиралась бы в предпросмотре и не собиралась в экспорте.
///
/// Байты PDF возвращаются только вместе с документом без ошибок: отказ,
/// дописанный после сборки (непрочитанный файл, неудачная установка), делает
/// документ несобранным, и файл из него писать нельзя (009 FR-008).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn build_with_templates<B: VaultBackend>(
    backend: &mut B,
    packages: &PackageCache,
    note_path: &str,
    source_text: &str,
    revision: u64,
    fonts: FontSet,
    policy: &TemplatePolicy,
    render: &Render,
) -> Result<(CompiledDocument, PdfBytes), SnapshotError> {
    let (document, pdf) = build_installing_templates(
        backend,
        packages,
        note_path,
        source_text,
        revision,
        fonts,
        policy,
        render,
    )
    .await?;
    let pdf = pdf.filter(|_| document.outcome == CompileOutcome::Succeeded);
    Ok((document, pdf))
}

#[allow(clippy::too_many_arguments)]
async fn build_installing_templates<B: VaultBackend>(
    backend: &mut B,
    packages: &PackageCache,
    note_path: &str,
    source_text: &str,
    revision: u64,
    fonts: FontSet,
    policy: &TemplatePolicy,
    render: &Render,
) -> Result<(CompiledDocument, PdfBytes), SnapshotError> {
    let (document, pdf, failures) = compile_collecting_failures(
        backend,
        packages,
        note_path,
        source_text,
        revision,
        fonts.clone(),
        render,
    )
    .await?;

    // Шаблоны, за которыми идти некуда (`@local/…` без указанного адреса),
    // отсеиваются сразу: для них поведение то же, что до фичи 007, — причина
    // «шаблона нет» уже стоит среди отказов, а сообщение о загрузке было бы
    // выдумкой о попытке, которой не было (FR-005, US1 AS4).
    let missing: Vec<String> = missing_templates(&failures)
        .into_iter()
        .filter(|spec| policy.has_source(spec))
        .collect();
    if missing.is_empty() {
        let mut document = with_resolve_failures(document, failures);
        document
            .diagnostics
            .extend(fingerprint_notes(backend, packages).await);
        sort_by_location(&mut document.diagnostics);
        return Ok((document, pdf));
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
                notes.push(Diagnostic::app(
                    DiagnosticSeverity::Warning,
                    AppMessage::TemplateDownloaded {
                        spec: report.spec,
                        url: report.url,
                        file_count: u32::try_from(report.file_count).unwrap_or(u32::MAX),
                        fingerprint: report.fingerprint,
                    },
                ));
            }
            Err(error) => {
                // FR-007: причина и тот адрес, к которому обращались.
                notes.push(Diagnostic {
                    // Кнопка «загрузить» в интерфейсе — по этому полю.
                    template: Some(spec.clone()),
                    ..Diagnostic::app(
                        DiagnosticSeverity::Error,
                        AppMessage::TemplateInstallFailed {
                            spec: spec.clone(),
                            reason: error,
                        },
                    )
                });
            }
        }
    }

    if !installed_any {
        let mut document = with_resolve_failures(document, failures);
        document.diagnostics.extend(notes);
        sort_by_location(&mut document.diagnostics);
        return Ok((document, pdf));
    }

    // Пересборка: теперь недостающие шаблоны на месте. Прежние причины отказов
    // не переносятся — они относились к состоянию до установки.
    let (document, pdf, failures) = compile_collecting_failures(
        backend,
        packages,
        note_path,
        source_text,
        revision,
        fonts,
        render,
    )
    .await?;
    let mut document = with_resolve_failures(document, failures);
    document.diagnostics.extend(notes);
    document
        .diagnostics
        .extend(fingerprint_notes(backend, packages).await);
    sort_by_location(&mut document.diagnostics);
    Ok((document, pdf))
}

/// Замечания о шаблонах, чьё содержимое разошлось с записанным отпечатком
/// (FR-023).
///
/// Ничего не переустанавливает: правка установленного шаблона может быть
/// намеренной, и молча вернуть её назад значило бы стереть работу пользователя.
/// Дело этой функции — сказать, что содержимое уже не то, которое было
/// получено.
///
/// Сверяется то, что документ действительно читал, и раз за сеанс: сверка
/// читает все файлы шаблона с диска, а на каждую сборку это вернуло бы те самые
/// чтения, ради устранения которых заведён кэш.
async fn fingerprint_notes<B: VaultBackend>(
    backend: &B,
    packages: &PackageCache,
) -> Vec<Diagnostic> {
    let mut notes = Vec::new();
    for spec in packages.specs() {
        if !packages.mark_verified(&spec) {
            continue;
        }
        let (recorded, actual) = match verify_installed(backend, &spec).await {
            FingerprintCheck::Diverged { recorded, actual } => {
                (recorded, FingerprintActual::Digest { digest: actual })
            }
            FingerprintCheck::FileMissing { recorded, name } => {
                (recorded, FingerprintActual::FileMissing { name })
            }
            FingerprintCheck::Matches | FingerprintCheck::Unknown => continue,
        };
        notes.push(Diagnostic::app(
            DiagnosticSeverity::Warning,
            AppMessage::TemplateFingerprintDiverged {
                spec: spec.to_string(),
                recorded,
                actual,
            },
        ));
    }
    notes
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

    /// Есть ли у шаблона источник: указанный адрес либо реестр, обслуживающий
    /// его пространство имён. Нет источника — нет и повода говорить о загрузке.
    fn has_source(&self, spec: &str) -> bool {
        parse_spec(spec)
            .and_then(|parsed| source_url(&parsed, &self.source_for(spec)))
            .is_some()
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

/// Установить шаблон по явному запросу пользователя (FR-010).
///
/// Отличается от пути сборки ровно одним: выключателя автозагрузки здесь нет.
/// Выключен **автоматический** поход в сеть, а не сама возможность получить
/// шаблон — иначе настройка запирала бы дверь, которую пользователь только что
/// попросил открыть.
///
/// Адрес по-прежнему берётся из настроек папки: указанный пользователем для
/// этого шаблона либо общий реестр (FR-004).
pub async fn install_template<B: VaultBackend>(
    backend: &mut B,
    spec_text: &str,
    policy: &TemplatePolicy,
) -> Result<InstallReport, InstallError> {
    let spec = parse_spec(spec_text).ok_or_else(|| InstallError::VersionNotFound {
        spec: spec_text.to_string(),
    })?;
    fetch_and_install(backend, &spec, &policy.source_for(spec_text)).await
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
///
/// Разбор один на модуль: реестр установленного хранит те же строки, и вторая
/// его копия разошлась бы с этой при первой же правке.
use super::packages::parse_spec;

/// Один проход сборки. Возвращает результат и снимок обратно — в нём
/// накопленные файлы и список того, чего не хватило.
fn compile_with(
    main: FileId,
    note_path: &str,
    snapshot: DependencySnapshot,
    revision: u64,
    fonts: FontSet,
    render: &Render,
) -> (CompiledDocument, PdfBytes, DependencySnapshot) {
    let world = PreviewWorld::new(main, snapshot, fonts.into_fonts());
    let compiled = typst::compile::<PagedDocument>(&world);

    let mut diagnostics: Vec<Diagnostic> = compiled
        .warnings
        .iter()
        .map(|d| translate(&world, main, d))
        .collect();

    let (outcome, pages, pdf) = match (compiled.output, render) {
        (Ok(document), Render::Pages) => (CompileOutcome::Succeeded, render_pages(&document), None),
        // Вывод в PDF тоже может отказать: например, шрифт без нужных таблиц
        // или изображение, которое экспорт не умеет встроить. Такой отказ —
        // диагностика того же вида, что ошибка сборки, и файл тогда не пишется.
        (Ok(document), Render::Pdf(meta)) => match typst_pdf::pdf(&document, &pdf_options(meta)) {
            Ok(bytes) => (CompileOutcome::Succeeded, Vec::new(), Some(bytes)),
            Err(errors) => {
                diagnostics.extend(errors.iter().map(|d| translate(&world, main, d)));
                (CompileOutcome::Failed, Vec::new(), None)
            }
        },
        (Err(errors), _) => {
            diagnostics.extend(errors.iter().map(|d| translate(&world, main, d)));
            (CompileOutcome::Failed, Vec::new(), None)
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
        pdf,
        world.into_snapshot(),
    )
}

/// Параметры PDF (009/research.md §2).
///
/// `timestamp: None` — дата документа берётся только из `set document(date:)`:
/// часов в wasm без JS нет, а одинаковый результат двух экспортов одной
/// заметки проверяется тестами побайтно.
fn pdf_options(meta: &PdfMeta) -> typst_pdf::PdfOptions {
    typst_pdf::PdfOptions {
        ident: typst::foundations::Smart::Custom(meta.ident.clone()),
        creator: typst::foundations::Smart::Custom(Some(PDF_CREATOR.to_owned())),
        timestamp: None,
        ..typst_pdf::PdfOptions::default()
    }
}

/// Метка создателя в метаданных PDF (009 FR-004).
pub(crate) const PDF_CREATOR: &str = "Typsidian (Typst 0.15.1)";

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
        // Места в тексте у такой ошибки нет: она про файл целиком, а не про
        // позицию в нём. Перехода к ней быть не должно (FR-033). У
        // отсутствующего шаблона имя несёт сама причина: путь внутри пакета
        // пользователю ничего не говорит, он писал в заметке `/…`
        // (фича 006, FR-037).
        document.diagnostics.push(Diagnostic::app(
            DiagnosticSeverity::Error,
            AppMessage::from_resolve(&path, &error),
        ));
    }
    document.outcome = CompileOutcome::Failed;
    document
}
