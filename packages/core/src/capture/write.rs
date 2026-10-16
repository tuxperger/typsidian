//! Запись захвата без потерь (research.md §6, FR-003, FR-004, FR-005).
//!
//! Устроено по образцу «Связать» (фича 013, `links::link_mention`): одна точка
//! решения в ядре сама выбирает между правкой буфера открытой заметки и
//! записью файла со снимком `expected`. Решать это в интерфейсе нельзя —
//! неверный выбор и есть потеря несохранённых правок (принцип I).
//!
//! # Что записывается в vault
//!
//! - заметка открыта в редакторе (`open_buffer`) — файл заметки **не
//!   трогается**: возвращается правка буфера, а сохранение идёт обычным путём;
//! - иначе — `write(path, текст, Some(снимок))`; расхождение снимка (заметку
//!   изменили на другом устройстве между чтением и записью) повторяет шаг на
//!   свежем тексте, до [`ATTEMPTS`] раз;
//! - картинка — отдельный файл вложения по правилам размещения 006; он пишется
//!   до заметки, и при отказе записи заметки остаётся на месте (повторный
//!   захват находит его по содержимому и не плодит копий).
//!
//! Отказ любого шага — [`CaptureOutcome::Failed`] с названной причиной; vault
//! при этом не тронут, кроме уже записанной картинки.

use std::path::{Path, PathBuf};

use super::daily::{ensure_parent, exists, open_daily, DailyError};
use super::entry::{append, entry};
use super::{
    capture_settings, normalize_path, Capture, CaptureContent, CaptureFailure, CaptureOutcome,
    CaptureSettings, CaptureTarget,
};
use crate::boards::placement::{note_assets_folder, relative_to};
use crate::vault::backend::{CreateError, ReadError, VaultBackend, WriteError};
use crate::vault::vault_settings::{read_vault_settings, VaultOverrides};
use crate::vault::{classify, AccessState, FileKind, NOTE_SIZE_LIMIT_BYTES};

/// Сколько раз повторять запись при расхождении снимка.
pub const ATTEMPTS: usize = 3;

/// Путь заметки назначения от корня vault, через `/`.
///
/// Нужен окну до записи: открыта ли эта заметка в редакторе, решает оно, а
/// ядро по ответу выбирает способ записи. `None` — назначение не заметка vault
/// (или дата не существует).
pub fn capture_target_path(settings: &CaptureSettings, capture: &Capture) -> Option<String> {
    match &capture.target {
        CaptureTarget::Inbox => Some(settings.inbox.clone()),
        CaptureTarget::Daily => super::daily_path(settings, &capture.at.date()),
        CaptureTarget::Note { path } => {
            let path = normalize_path(path).ok().flatten()?;
            (classify(Path::new(&path)) == FileKind::Note).then_some(path)
        }
    }
}

/// Дописывает захват в конец заметки назначения.
///
/// `open_buffer` — полный текст заметки в редакторе, если она там открыта.
/// `image` — байты картинки для [`CaptureContent::Image`].
pub async fn capture_write<B: VaultBackend>(
    backend: &mut B,
    capture: &Capture,
    open_buffer: Option<&str>,
    image: Option<&[u8]>,
) -> CaptureOutcome {
    match run(backend, capture, open_buffer, image).await {
        Ok(outcome) => outcome,
        Err(reason) => CaptureOutcome::Failed { reason },
    }
}

async fn run<B: VaultBackend>(
    backend: &mut B,
    capture: &Capture,
    open_buffer: Option<&str>,
    image: Option<&[u8]>,
) -> Result<CaptureOutcome, CaptureFailure> {
    match backend.access_state() {
        AccessState::ReadWrite => {}
        AccessState::ReadOnly => return Err(CaptureFailure::ReadOnly),
        AccessState::Unavailable => return Err(CaptureFailure::NoVault),
    }
    if is_empty(capture) {
        return Err(CaptureFailure::Empty);
    }

    // Битый файл настроек захват не останавливает: действуют умолчания.
    let overrides = read_vault_settings(backend).await.unwrap_or_default();
    let settings = capture_settings(overrides.capture.as_ref());
    let path = capture_target_path(&settings, capture).ok_or(CaptureFailure::InvalidTarget)?;

    // Негодная картинка отказывает до любых изменений vault: пустая заметка
    // входящих из-за неё появляться не должна.
    if matches!(capture.content, CaptureContent::Image { .. }) {
        check_image(image)?;
    }

    // Заметка в редакторе существует по определению; иначе при необходимости
    // создаём (входящие, ежедневная) либо отказываем (выбранная пропала).
    if open_buffer.is_none() && !exists(backend, &path).await {
        match &capture.target {
            CaptureTarget::Note { .. } => return Err(CaptureFailure::TargetMissing),
            CaptureTarget::Inbox => create_empty(backend, &path).await?,
            CaptureTarget::Daily => {
                open_daily(backend, &settings, &capture.at.date())
                    .await
                    .map_err(daily_failure)?;
            }
        }
    }

    let image_path = match &capture.content {
        CaptureContent::Image { .. } => {
            Some(store_image(backend, &overrides, &path, capture, image).await?)
        }
        _ => None,
    };
    let text = entry(&settings, capture, image_path.as_deref());

    if let Some(buffer) = open_buffer {
        let appended = append(buffer, settings.entry_style, &text);
        check_size(&appended.new_text)?;
        return Ok(CaptureOutcome::EditBuffer {
            path,
            edit: appended.edit,
        });
    }

    if backend.has_unsaved_edits(Path::new(&path)) {
        return Err(CaptureFailure::UnsavedElsewhere);
    }
    append_to_file(backend, &settings, &path, &text).await
}

/// Запись в файл: чтение со снимком, дописывание, запись с `expected`; при
/// расхождении снимка — заново на свежем тексте.
async fn append_to_file<B: VaultBackend>(
    backend: &mut B,
    settings: &CaptureSettings,
    path: &str,
    entry_text: &str,
) -> Result<CaptureOutcome, CaptureFailure> {
    let file = Path::new(path);
    let folder = file.parent().unwrap_or(Path::new(""));
    for _ in 0..ATTEMPTS {
        // Снимок берётся из индекса самого backend: сравним на обоих таргетах
        // только он (см. `links::link_mention`).
        let stamp = backend
            .list_folder(folder)
            .await
            .into_iter()
            .find(|note| note.path == file)
            .map(|note| note.stamp)
            .ok_or(CaptureFailure::TargetMissing)?;
        let text = backend.read(file).await.map_err(|error| match error {
            ReadError::NotFound => CaptureFailure::TargetMissing,
            ReadError::TooLarge => CaptureFailure::TooLarge,
            ReadError::Io => CaptureFailure::Io,
        })?;
        let appended = append(&text, settings.entry_style, entry_text);
        check_size(&appended.new_text)?;
        match backend.write(file, &appended.new_text, Some(stamp)).await {
            Ok(()) => {
                return Ok(CaptureOutcome::Written {
                    path: path.to_owned(),
                })
            }
            Err(WriteError::Conflict) => continue,
            Err(WriteError::AccessDenied) => return Err(CaptureFailure::ReadOnly),
            Err(WriteError::Io) => return Err(CaptureFailure::Io),
        }
    }
    Err(CaptureFailure::Conflict)
}

/// Предел размера заметки (001 FR-013): больше не записываем.
fn check_size(text: &str) -> Result<(), CaptureFailure> {
    if text.len() as u64 > NOTE_SIZE_LIMIT_BYTES {
        return Err(CaptureFailure::TooLarge);
    }
    Ok(())
}

fn is_empty(capture: &Capture) -> bool {
    let has_note = capture
        .note
        .as_deref()
        .is_some_and(|note| !note.trim().is_empty());
    match &capture.content {
        CaptureContent::Text { text } => text.trim().is_empty() && !has_note,
        CaptureContent::Link { url, .. } => url.trim().is_empty(),
        CaptureContent::Image { .. } => false,
    }
}

fn daily_failure(error: DailyError) -> CaptureFailure {
    match error {
        DailyError::InvalidDate => CaptureFailure::InvalidTarget,
        DailyError::ReadOnly => CaptureFailure::ReadOnly,
        DailyError::NoVault => CaptureFailure::NoVault,
        DailyError::Io => CaptureFailure::Io,
    }
}

/// Создаёт пустую заметку входящих. Чужое создание параллельно — не ошибка.
async fn create_empty<B: VaultBackend>(backend: &mut B, path: &str) -> Result<(), CaptureFailure> {
    // Входящие могут лежать в папке из настроек, которой ещё нет: `create`
    // папок не заводит.
    ensure_parent(backend, Path::new(path))
        .await
        .map_err(|error| match error {
            WriteError::AccessDenied => CaptureFailure::ReadOnly,
            WriteError::Conflict | WriteError::Io => CaptureFailure::Io,
        })?;
    match backend.create(Path::new(path)).await {
        Ok(()) | Err(CreateError::NameConflict) => Ok(()),
        Err(CreateError::AccessDenied) => Err(CaptureFailure::ReadOnly),
        Err(CreateError::InvalidName) => Err(CaptureFailure::InvalidTarget),
        Err(CreateError::Io) => Err(CaptureFailure::Io),
    }
}

// ── Вложение ────────────────────────────────────────────────────────────────

/// Расширение по содержимому: вид картинки определяют байты, а не подпись
/// отправителя (`image/*` из «Поделиться» бывает и HEIC, которого Typst не
/// читает). `None` — не картинка, которую умеет показать документ.
fn sniff_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
        let head = head.trim_start_matches('\u{feff}').trim_start();
        (head.starts_with("<svg") || (head.starts_with("<?xml") && head.contains("<svg")))
            .then_some("svg")
    }
}

/// Годится ли картинка: есть, не больше предела и вида, который читает Typst.
/// Возвращает расширение файла вложения.
fn check_image(bytes: Option<&[u8]>) -> Result<&'static str, CaptureFailure> {
    let bytes = bytes
        .filter(|bytes| !bytes.is_empty())
        .ok_or(CaptureFailure::BadImage)?;
    if bytes.len() as u64 > NOTE_SIZE_LIMIT_BYTES {
        return Err(CaptureFailure::TooLarge);
    }
    sniff_extension(bytes).ok_or(CaptureFailure::BadImage)
}

/// Сохраняет картинку как вложение и возвращает путь вставки относительно
/// заметки назначения (FR-006).
///
/// Папка — по политике размещения vault (006): та же, что у вставок досок.
/// Имя — `<заметка>-ГГГГ-ММ-ДД-ЧЧММ.<вид>`; занятое имя получает `-2`, `-3`, …
/// Если под кандидатом уже лежит файл с теми же байтами, он и берётся — так
/// повтор захвата после отказа не плодит копии.
async fn store_image<B: VaultBackend>(
    backend: &mut B,
    overrides: &VaultOverrides,
    note_path: &str,
    capture: &Capture,
    bytes: Option<&[u8]>,
) -> Result<String, CaptureFailure> {
    let extension = check_image(bytes)?;
    let bytes = bytes.ok_or(CaptureFailure::BadImage)?;

    let note = Path::new(note_path);
    let folder = note_assets_folder(overrides.placement(), note, &overrides.sections)
        .ok_or(CaptureFailure::InvalidTarget)?;
    let stem = note
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or(CaptureFailure::InvalidTarget)?;
    let at = &capture.at;
    let base = format!(
        "{stem}-{:04}-{:02}-{:02}-{:02}{:02}",
        at.year, at.month, at.day, at.hour, at.minute
    );

    let mut chosen: Option<(PathBuf, bool)> = None;
    for attempt in 1..=9999u32 {
        let name = if attempt == 1 {
            format!("{base}.{extension}")
        } else {
            format!("{base}-{attempt}.{extension}")
        };
        let candidate = folder.join(name);
        match backend.read_bytes(&candidate).await {
            Err(ReadError::NotFound) => {
                chosen = Some((candidate, false));
                break;
            }
            Ok(existing) if existing == bytes => {
                chosen = Some((candidate, true));
                break;
            }
            // Занято чужим файлом (или нечитаемым) — следующее имя.
            _ => {}
        }
    }
    let (target, already_there) = chosen.ok_or(CaptureFailure::Io)?;

    if !already_there {
        let denied = |error: WriteError| match error {
            WriteError::AccessDenied => CaptureFailure::ReadOnly,
            WriteError::Conflict | WriteError::Io => CaptureFailure::Io,
        };
        backend.ensure_folder(&folder).await.map_err(denied)?;
        backend.write_bytes(&target, bytes).await.map_err(denied)?;
    }
    let from = note.parent().unwrap_or(Path::new(""));
    Ok(relative_to(from, &target))
}
