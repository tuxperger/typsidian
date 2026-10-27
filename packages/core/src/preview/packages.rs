//! Получение и установка внешних шаблонов Typst (фича 007,
//! contracts/template-install.md).
//!
//! Фича 006 научила приложение **использовать** шаблоны, лежащие в
//! `.typsidian/packages/`, и внятно отказывать, когда их нет. Здесь — вторая
//! половина: недостающий шаблон скачивается и устанавливается.
//!
//! # Про платформенный шов
//!
//! `reqwest` сам является платформенной абстракцией: под native он работает
//! через `hyper`, под wasm32 — через `fetch` браузера, и `.timeout()` есть в
//! обоих бэкендах (research.md §1, §2). Поэтому отдельного трейта загрузчика с
//! двумя реализациями здесь нет — принцип II допускает такой трейт, но чем
//! меньше швов, тем меньше мест, где таргеты могут разойтись.
//!
//! **Одна `cfg`-ветка всё же осталась** — [`is_connect_failure`], и она
//! отражает настоящее различие платформ, а не разницу нашего кода: в браузере
//! отказ соединения и запрет по политике доступа приходят неразличимо, и
//! выдумывать различие, которого среда не даёт, нельзя. Ветка сидит в одной
//! функции на три строки, и правило вокруг неё общее.
//!
//! # Порядок, из которого следуют гарантии
//!
//! 1. скачать архив целиком в память;
//! 2. разобрать целиком в память, проверив **каждую** запись;
//! 3. только после этого начать запись файлов;
//! 4. манифест — последним.
//!
//! Шаг 3 делает FR-019 («ни одного файла за пределами папки версии») свойством
//! порядка, а не проверкой внутри цикла записи: пока разбор не закончен, писать
//! просто нечего. Шаг 4 даёт FR-020 («всё или ничего») без транзакций — см.
//! [`install`].

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use typst::syntax::package::PackageSpec;

use super::resolve::{download_root, installed_root, PACKAGE_MANIFEST};
use crate::vault::backend::{VaultBackend, WriteError};

/// Пространство имён, которое обслуживает общий реестр шаблонов.
///
/// Остальные (`local` и любые другие) остаются чисто локальными: в сеть за ними
/// идти некуда, и попытка была бы выдумкой (FR-005).
pub const REGISTRY_NAMESPACE: &str = "preview";

/// Адрес общего реестра.
const REGISTRY_BASE: &str = "https://packages.typst.org";

/// Предел ожидания одной попытки (FR-014).
///
/// Задаётся одним и тем же вызовом на обоих таргетах: под wasm `reqwest`
/// реализует его через `AbortController` (research.md §2). Значение выбрано
/// заметно больше типичной загрузки архива в единицы сотен килобайт, но
/// достаточно малым, чтобы отказ был именно отказом, а не «приложение висит».
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// Предел размера архива.
///
/// Наследуется от предела отдельного файла заметки фичи 001, а не вводится как
/// новая пользовательская гарантия (Assumptions спеки): смысл тот же —
/// не читать в память то, что в неё не помещается.
pub const ARCHIVE_SIZE_LIMIT: usize = crate::vault::NOTE_SIZE_LIMIT_BYTES as usize;

/// Имя файла, в котором приложение хранит отпечаток установленного шаблона.
///
/// Отдельным файлом, а не строкой внутри `typst.toml`: манифест принадлежит
/// автору шаблона, и дописывать в него своё приложение не имеет права — то же
/// правило, по которому заметка остаётся обычным Typst (фича 002, FR-022).
pub const FINGERPRINT_FILE: &str = ".typsidian-fingerprint";

/// Откуда брать архив шаблона (FR-003, FR-004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum TemplateSource {
    /// Общий реестр: адрес выводится из спецификации пакета.
    Registry,
    /// Адрес, указанный пользователем **для этого шаблона**.
    ///
    /// Именно для шаблона, а не как глобальная замена реестра: подмена реестра
    /// целиком превратила бы любую заметку в источник неожиданного кода
    /// (Assumptions спеки).
    Url { url: String },
}

/// Почему шаблон не установлен.
///
/// Каждая причина отдельна намеренно: общего «не удалось» быть не должно
/// (FR-007, SC-008), потому что чинятся они по-разному — одна правкой версии в
/// заметке, другая включением сети, третья вообще ничем.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum InstallError {
    /// Адрес назван и здесь: FR-007 требует его у **каждого** отказа, а
    /// «сети нет» без адреса не говорит, куда приложение пыталось обратиться.
    #[error("сети нет: {url} недостижим")]
    Offline { url: String },
    #[error("источник {url} недоступен")]
    SourceUnavailable { url: String },
    /// Среда клиента не дала обратиться к источнику (FR-015). В браузере это
    /// обычно политика доступа к чужому домену.
    #[error(
        "обращение к {url} запрещено средой этого клиента; шаблон можно положить \
         вручную в папку .typsidian/packages/ этой папки заметок"
    )]
    Blocked { url: String },
    #[error("в источнике нет версии {spec}")]
    VersionNotFound { spec: String },
    #[error("ожидание ответа от {url} исчерпано")]
    TimedOut { url: String },
    /// Архив отклонён целиком, ни одного файла не создано (FR-019, FR-020).
    #[error("архив отклонён: {reason}")]
    ArchiveRejected { reason: ArchiveProblem },
    #[error("папка заметок недоступна на запись")]
    NotWritable,
    /// Скачанное не удалось записать в папку заметок, хотя она доступна на
    /// запись (сбой диска, гонка с внешней правкой).
    #[error("файлы шаблона не удалось записать в папку заметок")]
    LocalWriteFailed,
    /// Автозагрузка выключена (FR-009). Не отказ сети: приложение и не пыталось.
    #[error("шаблон {spec} не установлен, а автозагрузка выключена")]
    AutoDownloadDisabled { spec: String },
}

/// Чем архив шаблона не устроил (FR-019, FR-020).
///
/// Вид, а не готовый текст (фича 010, FR-002): слова подбирает интерфейс на
/// своём языке. `Display` — только для журналов, он от языка интерфейса не
/// зависит (FR-031).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(
    tag = "problem",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ArchiveProblem {
    #[error("архив больше поддерживаемого размера")]
    TooLarge,
    #[error("архив не разбирается")]
    Unparsable,
    #[error("запись архива не имеет пригодного пути")]
    EntryWithoutPath,
    #[error("запись архива не читается")]
    EntryUnreadable,
    #[error("распакованный шаблон больше поддерживаемого размера")]
    UnpackedTooLarge,
    #[error("в архиве нет файлов")]
    Empty,
    #[error("в архиве нет манифеста {PACKAGE_MANIFEST}")]
    NoManifest,
    #[error("путь записи архива не является текстом")]
    PathNotText,
    /// Запись пытается выйти за пределы папки шаблона; `entry` — её путь как
    /// есть в архиве.
    #[error("запись «{entry}» ведёт за пределы папки шаблона")]
    EscapesFolder { entry: String },
    #[error("запись архива без имени")]
    EntryWithoutName,
}

/// Что получилось из попытки установить шаблон — для отчёта пользователю
/// (FR-006, FR-007).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct InstallReport {
    pub spec: String,
    pub url: String,
    pub fingerprint: String,
    pub file_count: usize,
}

/// Сошлось ли содержимое установленного шаблона с записанным отпечатком
/// (FR-023).
///
/// `Unknown` — не «сошлось» и не «разошлось»: шаблон разложен руками или
/// установлен прежней версией приложения, и сверять не с чем. Смешивать это
/// состояние с «сошлось» нельзя — оно означало бы гарантию, которой нет.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FingerprintCheck {
    Matches,
    Diverged {
        recorded: String,
        actual: String,
    },
    /// Одного из записанных файлов больше нет — это тоже расхождение.
    FileMissing {
        recorded: String,
        name: String,
    },
    Unknown,
}

/// Установленный шаблон — для перечня в интерфейсе (FR-022, US5 AS1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct InstalledTemplate {
    /// Спецификация строкой — та же, что видит пользователь: `@preview/codly:1.3.0`.
    pub spec: String,
    /// Адрес, с которого шаблон получен. Пусто — установлен не этим приложением.
    pub url: String,
    pub fingerprint: String,
    pub file_count: usize,
}

/// Реестр установленного — список, а не обход папок.
///
/// Обхода каталога у `VaultBackend` нет, и заводить его ради одного перечня
/// значило бы расширить поверхность ядра сильнее, чем стоит задача. Истина
/// по-прежнему на диске: каждая запись перед показом сверяется с наличием
/// манифеста, и удалённый руками шаблон из перечня исчезает (см.
/// [`installed_templates`]).
pub const REGISTRY_FILE: &str = "installed.json";

/// Адрес, к которому приложение обратится.
///
/// `None` — обращаться некуда: реестр обслуживает только своё пространство
/// имён, а для прочих пользователь адреса не указывал (FR-005). Это не ошибка,
/// а отсутствие источника.
///
/// Чистая функция: её результат показывается пользователю и до попытки, и в
/// сообщении об отказе, поэтому он обязан быть одинаковым в обоих случаях.
pub fn source_url(spec: &PackageSpec, source: &TemplateSource) -> Option<String> {
    match source {
        TemplateSource::Url { url } => Some(url.clone()),
        TemplateSource::Registry => {
            if spec.namespace.as_str() != REGISTRY_NAMESPACE {
                return None;
            }
            Some(format!(
                "{REGISTRY_BASE}/{}/{}-{}.{}.{}.tar.gz",
                spec.namespace,
                spec.name,
                spec.version.major,
                spec.version.minor,
                spec.version.patch
            ))
        }
    }
}

/// Разбирает архив **целиком в памяти**, проверяя каждую запись.
///
/// Ни одного обращения к файловой системе: на wasm32 её нет, а проверить весь
/// архив нужно до создания первого файла (FR-019, FR-020).
///
/// Возвращает пары «путь внутри шаблона → содержимое», отсортированные по пути:
/// от порядка записей в архиве отпечаток зависеть не должен (FR-024).
pub fn unpack(archive: &[u8]) -> Result<Vec<(String, Vec<u8>)>, InstallError> {
    if archive.len() > ARCHIVE_SIZE_LIMIT {
        return Err(InstallError::ArchiveRejected {
            reason: ArchiveProblem::TooLarge,
        });
    }

    let decoder = flate2::read::GzDecoder::new(archive);
    let mut tar = tar::Archive::new(decoder);
    let entries = tar.entries().map_err(|_| InstallError::ArchiveRejected {
        reason: ArchiveProblem::Unparsable,
    })?;

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total = 0usize;

    for entry in entries {
        let mut entry = entry.map_err(|_| InstallError::ArchiveRejected {
            reason: ArchiveProblem::Unparsable,
        })?;
        // Папки сами по себе не переносятся: их создаст запись файлов внутри.
        if entry.header().entry_type().is_dir() {
            continue;
        }
        let raw = entry.path().map_err(|_| InstallError::ArchiveRejected {
            reason: ArchiveProblem::EntryWithoutPath,
        })?;
        let path = contained_path(&raw)?;

        // Не больше, чем осталось до предела (+1 — чтобы превышение было видно):
        // запись-«бомба» не распаковывается в память целиком (фича 037).
        let mut bytes = Vec::new();
        let room = (ARCHIVE_SIZE_LIMIT - total) as u64 + 1;
        entry
            .by_ref()
            .take(room)
            .read_to_end(&mut bytes)
            .map_err(|_| InstallError::ArchiveRejected {
                reason: ArchiveProblem::EntryUnreadable,
            })?;

        total = total.saturating_add(bytes.len());
        if total > ARCHIVE_SIZE_LIMIT {
            return Err(InstallError::ArchiveRejected {
                reason: ArchiveProblem::UnpackedTooLarge,
            });
        }
        files.push((path, bytes));
    }

    if files.is_empty() {
        return Err(InstallError::ArchiveRejected {
            reason: ArchiveProblem::Empty,
        });
    }

    let files = strip_common_wrapper(files);
    if !files.iter().any(|(path, _)| path == PACKAGE_MANIFEST) {
        return Err(InstallError::ArchiveRejected {
            reason: ArchiveProblem::NoManifest,
        });
    }

    let mut files = files;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

/// Приводит путь записи архива к пути внутри папки версии.
///
/// Отклоняет **весь архив**, если запись пытается выйти наружу: `..`, корень
/// или префикс диска (FR-019). Отклонение именно архива, а не пропуск записи:
/// архив, который так себя ведёт, целиком недостоин доверия, а установленный
/// «частично безопасный» шаблон был бы худшим из исходов.
fn contained_path(raw: &Path) -> Result<String, InstallError> {
    let mut parts: Vec<String> = Vec::new();
    for component in raw.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_str().ok_or_else(|| InstallError::ArchiveRejected {
                    reason: ArchiveProblem::PathNotText,
                })?;
                parts.push(name.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(InstallError::ArchiveRejected {
                    reason: ArchiveProblem::EscapesFolder {
                        entry: raw.display().to_string(),
                    },
                });
            }
        }
    }
    if parts.is_empty() {
        return Err(InstallError::ArchiveRejected {
            reason: ArchiveProblem::EntryWithoutName,
        });
    }
    Ok(parts.join("/"))
}

/// Срезает общую папку-обёртку, если она есть.
///
/// Архивы реестра кладут файлы пакета в корень, но архив, собранный вручную,
/// часто обёрнут папкой. Без среза `lib.typ` оказался бы на уровень глубже, чем
/// его ищет компилятор, и шаблон установился бы нерабочим.
fn strip_common_wrapper(files: Vec<(String, Vec<u8>)>) -> Vec<(String, Vec<u8>)> {
    let first_segment = |path: &str| path.split('/').next().unwrap_or_default().to_string();

    let Some((first, _)) = files.first() else {
        return files;
    };
    // Обёртка есть только если каждый путь начинается с одного и того же
    // сегмента И ни один файл не лежит в корне архива.
    let candidate = first_segment(first);
    let wrapped = files
        .iter()
        .all(|(path, _)| path.contains('/') && first_segment(path) == candidate);
    if !wrapped {
        return files;
    }

    files
        .into_iter()
        .map(|(path, bytes)| {
            let stripped = path
                .strip_prefix(&candidate)
                .and_then(|rest| rest.strip_prefix('/'))
                .unwrap_or(&path)
                .to_string();
            (stripped, bytes)
        })
        .collect()
}

/// Отпечаток установленного содержимого (FR-022, FR-024).
///
/// Считается по **установленным файлам**, а не по архиву: сверять надо то, что
/// исполняется, а архив после установки не хранится.
///
/// Длина содержимого входит в хэш намеренно: без неё пары `("ab", "c")` и
/// `("a", "bc")` дали бы один отпечаток, и «одинаковые отпечатки означают
/// одинаковое содержимое» перестало бы быть правдой.
pub fn fingerprint(files: &[(String, Vec<u8>)]) -> String {
    let mut sorted: Vec<&(String, Vec<u8>)> = files.iter().collect();
    sorted.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = blake3::Hasher::new();
    for (path, bytes) in sorted {
        // Файл отпечатка в него самого не входит: иначе запись отпечатка
        // меняла бы отпечаток.
        if path == FINGERPRINT_FILE {
            continue;
        }
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    hasher.finalize().to_hex().to_string()
}

/// Записывает разобранный шаблон в vault.
///
/// # Почему это «всё или ничего» без транзакций
///
/// Манифест пишется **последним**. Признак «шаблон установлен» у фичи 006 —
/// это именно наличие манифеста (`resolve::read_dependency`), поэтому до его
/// появления установка для всей остальной системы не состоялась, сколько бы
/// файлов уже ни легло. Оборванная установка выглядит как отсутствие шаблона, а
/// не как рабочий шаблон с дырами (FR-020).
///
/// Приём не требует ни новых операций у трейта, ни нового состояния: он
/// переиспользует уже существующее правило детекции как границу транзакции.
pub async fn install<B: VaultBackend>(
    backend: &mut B,
    spec: &PackageSpec,
    files: &[(String, Vec<u8>)],
) -> Result<String, InstallError> {
    // Скачанное — в папку скачанного: она не синхронизируется (фича 041).
    let root = download_root(spec);
    let digest = fingerprint(files);

    // Манифест откладывается на конец — см. доктрину выше.
    let mut deferred: Option<&Vec<u8>> = None;
    for (path, bytes) in files {
        if path == PACKAGE_MANIFEST {
            deferred = Some(bytes);
            continue;
        }
        write_file(backend, &root.join(path), bytes).await?;
    }

    // В файл отпечатка идёт и состав установленного: сверять отпечаток нужно по
    // тому же набору файлов, по которому он считался, а обхода каталога у
    // бэкенда нет — см. `recorded_fingerprint`.
    let mut record = String::with_capacity(digest.len() + files.len() * 16);
    record.push_str(&digest);
    for (path, _) in files {
        record.push('\n');
        record.push_str(path);
    }
    write_file(backend, &root.join(FINGERPRINT_FILE), record.as_bytes()).await?;

    let manifest = deferred.ok_or_else(|| InstallError::ArchiveRejected {
        reason: ArchiveProblem::NoManifest,
    })?;
    write_file(backend, &root.join(PACKAGE_MANIFEST), manifest).await?;

    Ok(digest)
}

/// Отпечаток, записанный при установке. `None` — шаблон установлен прежней
/// версией приложения либо разложен пользователем вручную.
///
/// Первая строка файла — сам отпечаток; дальше идут пути установленных файлов.
/// Формат сложился таким, потому что сверять отпечаток нужно по тому же набору
/// файлов, по которому он считался, а перечислить содержимое папки
/// `VaultBackend` не умеет — обхода каталога у него нет. Файл прежнего формата
/// (один отпечаток без списка) читается по-прежнему: сверить по нему нельзя,
/// и [`verify_installed`] честно отвечает `Unknown`.
pub async fn recorded_fingerprint<B: VaultBackend>(
    backend: &B,
    spec: &PackageSpec,
) -> Option<String> {
    read_record(backend, spec).await.map(|record| record.digest)
}

/// Записанное про установленный шаблон: отпечаток и состав.
struct InstallRecord {
    digest: String,
    files: Vec<String>,
}

async fn read_record<B: VaultBackend>(backend: &B, spec: &PackageSpec) -> Option<InstallRecord> {
    let path = installed_root(backend, spec).await?.join(FINGERPRINT_FILE);
    let text = backend.read(&path).await.ok()?;
    let mut lines = text.lines();
    let digest = lines.next()?.trim().to_string();
    if digest.is_empty() {
        return None;
    }
    Some(InstallRecord {
        digest,
        files: lines
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect(),
    })
}

/// Сверяет содержимое установленного шаблона с записанным отпечатком (FR-023).
///
/// Ничего не переписывает и не переустанавливает: правка шаблона руками может
/// быть намеренной, и решение остаётся за пользователем. Дело этой функции —
/// заметить расхождение, а не устранить его.
pub async fn verify_installed<B: VaultBackend>(
    backend: &B,
    spec: &PackageSpec,
) -> FingerprintCheck {
    let Some(record) = read_record(backend, spec).await else {
        return FingerprintCheck::Unknown;
    };
    if record.files.is_empty() {
        // Файл прежнего формата: отпечаток есть, состав неизвестен — пересчитать
        // не из чего.
        return FingerprintCheck::Unknown;
    }

    let Some(root) = installed_root(backend, spec).await else {
        return FingerprintCheck::Unknown;
    };
    let mut files: Vec<(String, Vec<u8>)> = Vec::with_capacity(record.files.len());
    for name in &record.files {
        match backend.read_bytes(&root.join(name)).await {
            Ok(bytes) => files.push((name.clone(), bytes)),
            // Файл исчез — это тоже расхождение, и молчать о нём нельзя.
            Err(_) => {
                return FingerprintCheck::FileMissing {
                    recorded: record.digest,
                    name: name.clone(),
                }
            }
        }
    }

    let actual = fingerprint(&files);
    if actual == record.digest {
        FingerprintCheck::Matches
    } else {
        FingerprintCheck::Diverged {
            recorded: record.digest,
            actual,
        }
    }
}

/// Перечень установленных шаблонов (US5 AS1).
///
/// Реестр — лишь список известных записей; истина на диске. Поэтому каждая
/// запись перед выдачей проверяется наличием манифеста: шаблон, удалённый
/// руками, из перечня исчезает, а не висит призраком.
pub async fn installed_templates<B: VaultBackend>(backend: &B) -> Vec<InstalledTemplate> {
    // Перечень скачанного и перечень прежних версий (до фичи 041 он лежал рядом
    // с ручными шаблонами); запись о той же версии берётся из нового.
    let mut entries: Vec<InstalledTemplate> = Vec::new();
    for path in [registry_path(), legacy_registry_path()] {
        let Ok(text) = backend.read(&path).await else {
            continue;
        };
        for entry in serde_json::from_str::<Vec<InstalledTemplate>>(&text).unwrap_or_default() {
            if !entries.iter().any(|known| known.spec == entry.spec) {
                entries.push(entry);
            }
        }
    }
    entries.sort_by(|a, b| a.spec.cmp(&b.spec));

    let mut alive = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some(spec) = parse_spec(&entry.spec) else {
            continue;
        };
        if installed_root(backend, &spec).await.is_some() {
            alive.push(entry);
        }
    }
    alive
}

/// Перечень скачанного — в папке скачанного: он принадлежит устройству.
fn registry_path() -> PathBuf {
    crate::vault::service_dir::downloads_dir().join(REGISTRY_FILE)
}

/// Где перечень лежал до фичи 041. Только читается.
fn legacy_registry_path() -> PathBuf {
    crate::vault::service_dir::packages_dir().join(REGISTRY_FILE)
}

/// Дописывает запись в реестр установленного, заменяя прежнюю о той же версии.
///
/// Отдельная операция, а не часть [`install`]: адрес источника знает только
/// вызывающая сторона — сама установка работает с уже полученными байтами и о
/// том, откуда они взялись, не осведомлена.
///
/// Отказ записи реестра установку не отменяет: шаблон уже на диске и работает,
/// а реестр — удобство перечня. Молча ронять установку из-за него было бы
/// хуже, чем показать неполный список.
pub async fn remember_installed<B: VaultBackend>(backend: &mut B, entry: InstalledTemplate) {
    let mut entries = match backend.read(&registry_path()).await {
        Ok(text) => serde_json::from_str::<Vec<InstalledTemplate>>(&text).unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    entries.retain(|known| known.spec != entry.spec);
    entries.push(entry);
    entries.sort_by(|a, b| a.spec.cmp(&b.spec));

    if let Ok(text) = serde_json::to_string_pretty(&entries) {
        let path = registry_path();
        if let Some(folder) = path.parent() {
            let _ = backend.ensure_folder(folder).await;
        }
        let _ = backend.write(&path, &text, None).await;
    }
}

/// Разбирает `@namespace/name:major.minor.patch` обратно в спецификацию.
///
/// Нужен реестру: записи хранятся строкой — ровно той, что видит пользователь.
pub fn parse_spec(text: &str) -> Option<PackageSpec> {
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
        version: typst::syntax::package::PackageVersion {
            major,
            minor,
            patch,
        },
    })
}

async fn write_file<B: VaultBackend>(
    backend: &mut B,
    path: &Path,
    bytes: &[u8],
) -> Result<(), InstallError> {
    if let Some(folder) = path.parent() {
        backend
            .ensure_folder(folder)
            .await
            .map_err(to_install_error)?;
    }
    // Байтами, а не текстом. Шаблон Typst — это не только `.typ`: в нём лежат
    // картинки и шрифты, и пропускать их через `String` нельзя, они перестанут
    // быть собой. Проверено на живом шаблоне: `codly` содержит
    // `src/typst-small.png`, и запись текстом отклоняла установку целиком.
    backend
        .write_bytes(path, bytes)
        .await
        .map_err(to_install_error)
}

fn to_install_error(error: WriteError) -> InstallError {
    match error {
        WriteError::AccessDenied => InstallError::NotWritable,
        WriteError::Conflict | WriteError::Io => InstallError::LocalWriteFailed,
    }
}

/// Скачивает архив.
///
/// Тонкая оболочка над одним вызовом: вся логика фичи живёт в чистых функциях
/// выше и проверяется без сети. Различие таргетов скрыто внутри `reqwest`, а не
/// здесь — ни одной `cfg`-ветки в этом файле нет.
pub async fn fetch(url: &str) -> Result<Vec<u8>, InstallError> {
    let client = reqwest::Client::builder();
    // `timeout` у билдера клиента под wasm не поддерживается; предел задаётся
    // на запросе — там он есть в обоих бэкендах (research.md §2).
    let client = client.build().map_err(|_| InstallError::Blocked {
        url: url.to_string(),
    })?;

    let response = client
        .get(url)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|error| classify_transport(error, url))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(InstallError::VersionNotFound {
            spec: url.to_string(),
        });
    }
    if !response.status().is_success() {
        return Err(InstallError::SourceUnavailable {
            url: url.to_string(),
        });
    }

    // Предел — до чтения тела: ответ на гигабайты не должен попасть в память
    // (на сервере, фича 037, это отказ всем аккаунтам сразу).
    let too_large = || InstallError::ArchiveRejected {
        reason: ArchiveProblem::TooLarge,
    };
    if response
        .content_length()
        .is_some_and(|length| length > ARCHIVE_SIZE_LIMIT as u64)
    {
        return Err(too_large());
    }
    read_limited(response, url, too_large).await
}

/// Тело ответа не больше [`ARCHIVE_SIZE_LIMIT`]: по частям, с отказом на первой
/// лишней. В браузере частей нет — там предел держит `Content-Length` выше и
/// проверка после чтения.
#[cfg(not(target_arch = "wasm32"))]
async fn read_limited(
    mut response: reqwest::Response,
    url: &str,
    too_large: impl Fn() -> InstallError,
) -> Result<Vec<u8>, InstallError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| classify_transport(error, url))?
    {
        if bytes.len() + chunk.len() > ARCHIVE_SIZE_LIMIT {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(target_arch = "wasm32")]
async fn read_limited(
    response: reqwest::Response,
    url: &str,
    too_large: impl Fn() -> InstallError,
) -> Result<Vec<u8>, InstallError> {
    let bytes = response
        .bytes()
        .await
        .map_err(|error| classify_transport(error, url))?;
    if bytes.len() > ARCHIVE_SIZE_LIMIT {
        return Err(too_large());
    }
    Ok(bytes.to_vec())
}

/// Переводит отказ транспорта в причину, которую можно назвать пользователю.
///
/// Общего «не удалось» здесь быть не должно (SC-008): «сети нет», «ожидание
/// исчерпано» и «среда запретила» чинятся по-разному, и пользователь обязан
/// понимать, чем именно.
/// Отличим ли отказ соединения от прочих транспортных отказов.
///
/// Единственная `cfg`-ветка модуля, и она про **платформу, а не про наш код**.
/// `reqwest::Error::is_connect` существует только в native-бэкенде: в браузере
/// `fetch` отдаёт неудачу без различения «хост недостижим» и «политика доступа
/// не пустила», и API это различие не возвращает.
///
/// Поэтому на web «сети нет» не диагностируется отдельно — такой отказ
/// попадает в [`InstallError::Blocked`], чья формулировка честно называет
/// обе возможности сразу. Выдумывать различие, которого среда не даёт, значило
/// бы сообщать пользователю то, чего мы не знаем.
#[cfg(not(target_arch = "wasm32"))]
fn is_connect_failure(error: &reqwest::Error) -> bool {
    error.is_connect()
}

#[cfg(target_arch = "wasm32")]
fn is_connect_failure(_error: &reqwest::Error) -> bool {
    false
}

fn classify_transport(error: reqwest::Error, url: &str) -> InstallError {
    if error.is_timeout() {
        return InstallError::TimedOut {
            url: url.to_string(),
        };
    }
    if is_connect_failure(&error) {
        return InstallError::Offline {
            url: url.to_string(),
        };
    }
    // В браузере отказ по политике доступа к чужому домену приходит как
    // обычная ошибка запроса без статуса — отличить её от прочих транспортных
    // отказов средствами `reqwest` нельзя, поэтому формулировка называет и то,
    // и другое: обратиться к источнику не дала среда клиента.
    if error.is_request() {
        return InstallError::Blocked {
            url: url.to_string(),
        };
    }
    InstallError::SourceUnavailable {
        url: url.to_string(),
    }
}

/// Получить и установить шаблон целиком (FR-001).
///
/// Порядок шагов и есть выполнение FR-019 и FR-020: запись начинается только
/// после того, как весь архив разобран и проверен.
pub async fn fetch_and_install<B: VaultBackend>(
    backend: &mut B,
    spec: &PackageSpec,
    source: &TemplateSource,
) -> Result<InstallReport, InstallError> {
    let url = source_url(spec, source).ok_or_else(|| InstallError::VersionNotFound {
        spec: spec.to_string(),
    })?;

    let archive = fetch(&url).await?;
    let files = unpack(&archive)?;
    let digest = install(backend, spec, &files).await?;

    let report = InstallReport {
        spec: spec.to_string(),
        url,
        fingerprint: digest,
        file_count: files.len(),
    };
    remember_installed(
        backend,
        InstalledTemplate {
            spec: report.spec.clone(),
            url: report.url.clone(),
            fingerprint: report.fingerprint.clone(),
            file_count: report.file_count,
        },
    )
    .await;

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use typst::syntax::package::PackageVersion;

    fn spec(namespace: &str, name: &str) -> PackageSpec {
        PackageSpec {
            namespace: namespace.into(),
            name: name.into(),
            version: PackageVersion {
                major: 1,
                minor: 3,
                patch: 0,
            },
        }
    }

    #[test]
    fn the_registry_url_follows_the_official_layout() {
        assert_eq!(
            source_url(&spec("preview", "codly"), &TemplateSource::Registry).as_deref(),
            Some("https://packages.typst.org/preview/codly-1.3.0.tar.gz")
        );
    }

    #[test]
    fn other_namespaces_have_nowhere_to_go() {
        // FR-005: реестр обслуживает только своё пространство имён, и попытка
        // сходить в сеть за `@local` была бы выдумкой.
        assert_eq!(
            source_url(&spec("local", "мой"), &TemplateSource::Registry),
            None
        );
    }

    #[test]
    fn an_explicit_url_wins_over_the_registry() {
        // FR-004: подставлять реестр вместо указанного адреса нельзя.
        let source = TemplateSource::Url {
            url: "https://example.org/шаблон.tar.gz".to_string(),
        };
        assert_eq!(
            source_url(&spec("preview", "codly"), &source).as_deref(),
            Some("https://example.org/шаблон.tar.gz")
        );
        // И даже для пространства имён, которого реестр не знает.
        assert!(source_url(&spec("local", "мой"), &source).is_some());
    }

    #[test]
    fn a_path_leaving_the_template_folder_is_rejected() {
        // FR-019. Отклоняется весь архив, а не отдельная запись.
        for evil in ["../снаружи.typ", "a/../../снаружи.typ", "/etc/passwd"] {
            let error = contained_path(Path::new(evil)).expect_err(evil);
            assert!(
                matches!(error, InstallError::ArchiveRejected { .. }),
                "{evil}"
            );
        }
    }

    #[test]
    fn an_ordinary_path_survives_normalisation() {
        assert_eq!(contained_path(Path::new("./lib.typ")).unwrap(), "lib.typ");
        assert_eq!(
            contained_path(Path::new("src/помощь.typ")).unwrap(),
            "src/помощь.typ"
        );
    }

    #[test]
    fn the_fingerprint_ignores_entry_order() {
        // FR-024: одинаковое содержимое — одинаковый отпечаток, независимо от
        // порядка записей в архиве.
        let forward = vec![
            ("a.typ".to_string(), b"one".to_vec()),
            ("b.typ".to_string(), b"two".to_vec()),
        ];
        let backward = vec![
            ("b.typ".to_string(), b"two".to_vec()),
            ("a.typ".to_string(), b"one".to_vec()),
        ];
        assert_eq!(fingerprint(&forward), fingerprint(&backward));
    }

    #[test]
    fn the_fingerprint_notices_a_single_byte() {
        let before = vec![("a.typ".to_string(), b"one".to_vec())];
        let after = vec![("a.typ".to_string(), b"onE".to_vec())];
        assert_ne!(fingerprint(&before), fingerprint(&after));
    }

    #[test]
    fn content_lengths_participate_so_boundaries_cannot_shift() {
        // Без длины в хэше эти два набора дали бы один отпечаток.
        let left = vec![
            ("a".to_string(), b"ab".to_vec()),
            ("b".to_string(), b"c".to_vec()),
        ];
        let right = vec![
            ("a".to_string(), b"a".to_vec()),
            ("b".to_string(), b"bc".to_vec()),
        ];
        assert_ne!(fingerprint(&left), fingerprint(&right));
    }

    #[test]
    fn the_fingerprint_file_does_not_hash_itself() {
        let without = vec![("lib.typ".to_string(), b"x".to_vec())];
        let mut with = without.clone();
        with.push((
            FINGERPRINT_FILE.to_string(),
            "что угодно".as_bytes().to_vec(),
        ));
        assert_eq!(fingerprint(&without), fingerprint(&with));
    }

    #[test]
    fn a_common_wrapper_folder_is_stripped() {
        let wrapped = vec![
            ("codly-1.3.0/lib.typ".to_string(), b"x".to_vec()),
            ("codly-1.3.0/typst.toml".to_string(), b"y".to_vec()),
        ];
        let flat = strip_common_wrapper(wrapped);
        assert_eq!(flat[0].0, "lib.typ");
        assert_eq!(flat[1].0, "typst.toml");
    }

    #[test]
    fn files_at_the_archive_root_are_left_alone() {
        // Архивы реестра устроены именно так — срезать здесь нечего.
        let flat = vec![
            ("lib.typ".to_string(), b"x".to_vec()),
            ("typst.toml".to_string(), b"y".to_vec()),
        ];
        assert_eq!(strip_common_wrapper(flat.clone()), flat);
    }
}
