//! Ежедневная заметка: путь по дате, соседние дни, создание (research.md §4,
//! §5).
//!
//! Правила названия — чистые функции ([`daily_path`], [`daily_date_of`]), а
//! поверх `VaultBackend` — только поиск и создание файла. Дату приносит окно
//! по часам устройства; «сегодня» в ядре не вычисляется.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{CaptureSettings, DailyDate, DailyName};
use crate::templates::{apply_insertion, read_template, InsertionPoint};
use crate::vault::backend::{CreateError, VaultBackend, WriteError};
use crate::vault::{classify, AccessState, FileKind};

/// Путь ежедневной заметки на дату, от корня vault, через `/`.
///
/// `None` — такого дня нет в календаре.
pub fn daily_path(settings: &CaptureSettings, date: &DailyDate) -> Option<String> {
    if !date.is_valid() {
        return None;
    }
    let (y, m, d) = (date.year, date.month, date.day);
    let relative = match settings.daily_name {
        DailyName::Iso => format!("{y:04}-{m:02}-{d:02}"),
        DailyName::IsoNested => format!("{y:04}/{m:02}/{y:04}-{m:02}-{d:02}"),
        DailyName::Dotted => format!("{d:02}.{m:02}.{y:04}"),
    };
    Some(format!("{}/{relative}.typ", settings.daily_folder))
}

/// Дата ежедневной заметки по её пути. `None` — это не ежедневная заметка
/// при действующих настройках (другая папка, другой вид названия).
///
/// Обратная к [`daily_path`] и проверяется ею: путь признаётся ежедневной
/// заметкой, только если именно он получается из найденной даты, — вложенные
/// папки неверного вида и «лишние» нули отсеиваются сами.
pub fn daily_date_of(settings: &CaptureSettings, path: &str) -> Option<DailyDate> {
    let stem = Path::new(path).file_stem()?.to_str()?;
    let digits = |part: &str, len: usize| {
        (part.len() == len && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<u16>().ok())
            .flatten()
    };
    let date = match settings.daily_name {
        DailyName::Iso | DailyName::IsoNested => {
            let mut parts = stem.split('-');
            let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
            if parts.next().is_some() {
                return None;
            }
            (digits(year, 4)?, digits(month, 2)?, digits(day, 2)?)
        }
        DailyName::Dotted => {
            let mut parts = stem.split('.');
            let (day, month, year) = (parts.next()?, parts.next()?, parts.next()?);
            if parts.next().is_some() {
                return None;
            }
            (digits(year, 4)?, digits(month, 2)?, digits(day, 2)?)
        }
    };
    let date = DailyDate {
        year: date.0,
        month: u8::try_from(date.1).ok()?,
        day: u8::try_from(date.2).ok()?,
    };
    (daily_path(settings, &date).as_deref() == Some(path)).then_some(date)
}

/// Ближайшие существующие ежедневные заметки в обе стороны (FR-008).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DailyNeighbours {
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub previous: Option<String>,
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub next: Option<String>,
}

/// Соседние дни: ближайшая существующая заметка раньше и позже `date`,
/// пропуски дней не мешают. Сама `date` не нужна ни в одной из сторон.
///
/// Ищет по индексу vault среди заметок с названием вида из настроек; заметки
/// другого вида названия (после смены настройки) не считаются — они остаются
/// на месте нетронутыми (US3, AS1).
pub async fn daily_neighbours<B: VaultBackend>(
    backend: &B,
    settings: &CaptureSettings,
    date: &DailyDate,
) -> DailyNeighbours {
    let mut previous: Option<DailyDate> = None;
    let mut next: Option<DailyDate> = None;
    for note in backend.list().await {
        if classify(&note.path) != FileKind::Note {
            continue;
        }
        let path = note.path.to_string_lossy().replace('\\', "/");
        let Some(found) = daily_date_of(settings, &path) else {
            continue;
        };
        if found < *date && previous.is_none_or(|best| found > best) {
            previous = Some(found);
        }
        if found > *date && next.is_none_or(|best| found < best) {
            next = Some(found);
        }
    }
    DailyNeighbours {
        previous: previous.and_then(|found| daily_path(settings, &found)),
        next: next.and_then(|found| daily_path(settings, &found)),
    }
}

/// Что стало с заготовкой при создании.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum DailyTemplateUse {
    /// Заготовка в настройках не указана (или заметка уже существовала).
    None,
    /// Заметка создана по заготовке.
    Applied,
    /// Заготовка указана, но не читается: заметка создана с заголовком, а
    /// «Сегодня» не отказано.
    Missing,
}

/// Результат «Сегодня».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct OpenDaily {
    pub path: String,
    /// Заметка создана этим вызовом.
    pub created: bool,
    pub template: DailyTemplateUse,
}

/// Почему ежедневную заметку открыть нельзя.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum DailyError {
    #[error("такого дня нет в календаре")]
    InvalidDate,
    /// Заметки нет, а vault только для чтения или недоступен.
    #[error("заметку создать нельзя: vault только для чтения")]
    ReadOnly,
    #[error("vault недоступен")]
    NoVault,
    #[error("не удалось создать заметку")]
    Io,
}

/// Открывает ежедневную заметку на дату, создавая её при отсутствии (FR-007).
///
/// Повторный вызов за те же сутки находит тот же файл и ничего не создаёт
/// (SC-004). Создание не перезаписывает: если файл появился между проверкой и
/// созданием (другое устройство, второе окно), он остаётся как есть.
///
/// Тело новой заметки — заготовка (003) дословно либо, без заготовки, заголовок
/// `= <название>`: слов на языке интерфейса у ядра нет, а результат обязан
/// совпадать во всех оболочках (FR-011).
pub async fn open_daily<B: VaultBackend>(
    backend: &mut B,
    settings: &CaptureSettings,
    date: &DailyDate,
) -> Result<OpenDaily, DailyError> {
    let path = daily_path(settings, date).ok_or(DailyError::InvalidDate)?;
    if exists(backend, &path).await {
        return Ok(OpenDaily {
            path,
            created: false,
            template: DailyTemplateUse::None,
        });
    }
    match backend.access_state() {
        AccessState::ReadWrite => {}
        AccessState::ReadOnly => return Err(DailyError::ReadOnly),
        AccessState::Unavailable => return Err(DailyError::NoVault),
    }

    let (content, template) = new_note_body(backend, settings, &path).await;
    // Папка ежедневных заметок названа в настройках, поэтому заводится явно:
    // `create` папок не создаёт (паритет native и web), и в свежем vault
    // «Сегодня» иначе отказало бы на первом же нажатии.
    ensure_parent(backend, Path::new(&path))
        .await
        .map_err(|error| match error {
            WriteError::AccessDenied => DailyError::ReadOnly,
            WriteError::Conflict | WriteError::Io => DailyError::Io,
        })?;
    match backend.create(Path::new(&path)).await {
        Ok(()) => {}
        // Заметку успели создать — открываем её, не трогая.
        Err(CreateError::NameConflict) => {
            return Ok(OpenDaily {
                path,
                created: false,
                template: DailyTemplateUse::None,
            })
        }
        Err(CreateError::AccessDenied) => return Err(DailyError::ReadOnly),
        Err(CreateError::InvalidName | CreateError::Io) => return Err(DailyError::Io),
    }
    if let Err(error) = backend.write(Path::new(&path), &content, None).await {
        // Пустой файл вместо заметки по заготовке оставлять нельзя: следующее
        // «Сегодня» нашло бы его и открыло пустым.
        let _ = backend.delete(Path::new(&path)).await;
        return Err(match error {
            WriteError::AccessDenied => DailyError::ReadOnly,
            WriteError::Conflict | WriteError::Io => DailyError::Io,
        });
    }
    Ok(OpenDaily {
        path,
        created: true,
        template,
    })
}

/// Заводит папку, в которой ляжет заметка; корень vault не трогается.
pub(crate) async fn ensure_parent<B: VaultBackend>(
    backend: &mut B,
    path: &Path,
) -> Result<(), WriteError> {
    match path.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => backend.ensure_folder(folder).await,
        _ => Ok(()),
    }
}

/// Есть ли файл по этому пути в индексе vault.
pub(crate) async fn exists<B: VaultBackend>(backend: &B, path: &str) -> bool {
    let folder = Path::new(path).parent().unwrap_or(Path::new(""));
    backend
        .list_folder(folder)
        .await
        .iter()
        .any(|note| note.path.as_path() == Path::new(path))
}

/// Текст новой заметки и судьба заготовки.
async fn new_note_body<B: VaultBackend>(
    backend: &B,
    settings: &CaptureSettings,
    path: &str,
) -> (String, DailyTemplateUse) {
    if let Some(template) = &settings.daily_template {
        if let Ok(content) = read_template(backend, Path::new(template)).await {
            let at = InsertionPoint {
                offset: 0,
                selection: None,
            };
            return (
                apply_insertion("", &at, &content),
                DailyTemplateUse::Applied,
            );
        }
        return (default_body(path), DailyTemplateUse::Missing);
    }
    (default_body(path), DailyTemplateUse::None)
}

fn default_body(path: &str) -> String {
    let title = Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    format!("= {title}\n")
}
