//! Операции над досками (фича 002, contracts/board-store-trait.md).
//!
//! Не отдельный трейт с парой реализаций, а функции поверх `VaultBackend`:
//! логика досок платформенно нейтральна, а всё, что различается по платформам —
//! чтение и запись файлов, — уже скрыто трейтом из 001. Вторая пара реализаций
//! означала бы второе место, где эти правила могут разойтись (принцип II).
//!
//! Что здесь есть и чего нет: гейты записи, валидация имени, пустая доска. Ни
//! рисования, ни экспорта, ни разбора модели Excalidraw — это работа движка
//! (research.md §9).

use super::digest::DigestError;
use super::{Board, BoardContent, ReadBoardError, WriteBoardError};
use crate::stamp::FileStamp;
use crate::vault::backend::{
    valid_note_name, CreateError, ReadError, VaultBackend, WriteError,
};
use crate::vault::{classify, FileKind};
use std::path::Path;

/// Может ли клиент писать доски.
///
/// FR-035 требует, чтобы просмотр на мобильном не изменял файл. Гарантия лежит
/// здесь, а не во флаге интерфейса: `viewModeEnabled` у движка убирает
/// инструменты, но не отменяет автосохранение, а отсутствие пути записи —
/// отменяет (research.md §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardWriteProfile {
    ReadWrite,
    ViewOnly,
}

/// Содержимое новой пустой доски.
///
/// Пишется ровно то, что написал бы сам движок: своих полей приложение в файл
/// доски не добавляет (research.md §2). Пустая доска — штатный объект, её можно
/// вставить в заметку (US2 AS3).
pub const EMPTY_BOARD: &str =
    r#"{"type":"excalidraw","version":2,"source":"typsidian","elements":[],"appState":{},"files":{}}"#;

/// Почему доска не создана.
#[derive(
    Debug, Clone, PartialEq, Eq, thiserror::Error, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum CreateBoardError {
    #[error("имя уже занято в этой папке")]
    NameConflict,
    #[error("имя недопустимо для файловой системы")]
    InvalidName,
    /// Путь не оканчивается на расширение доски: создавать доску под именем
    /// заметки нельзя — классификатор отнёс бы её не к тому виду.
    #[error("это имя не является именем доски")]
    NotABoardName,
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("не удалось создать файл")]
    Io,
}

/// Создать пустую доску (FR-001).
///
/// Имя проверяется до любого обращения к диску — тем же правилом, что и для
/// заметок (001, FR-013): валидация не дублируется, а переиспользуется.
pub async fn create_board<B: VaultBackend>(
    backend: &mut B,
    path: &Path,
) -> Result<Board, CreateBoardError> {
    let Some(name) = valid_note_name(path) else {
        return Err(CreateBoardError::InvalidName);
    };
    if classify(path) != FileKind::Board {
        return Err(CreateBoardError::NotABoardName);
    }
    let name = name.to_string();

    backend.create(path).await.map_err(|error| match error {
        CreateError::NameConflict => CreateBoardError::NameConflict,
        CreateError::InvalidName => CreateBoardError::InvalidName,
        CreateError::AccessDenied => CreateBoardError::AccessDenied,
        CreateError::Io => CreateBoardError::Io,
    })?;

    backend
        .write(path, EMPTY_BOARD, None)
        .await
        .map_err(|error| match error {
            WriteError::AccessDenied => CreateBoardError::AccessDenied,
            WriteError::Conflict | WriteError::Io => CreateBoardError::Io,
        })?;

    Ok(Board {
        path: path.to_path_buf(),
        name,
        stamp: FileStamp::from_content(0, EMPTY_BOARD.as_bytes()),
        has_unsaved_edits: false,
    })
}

/// Прочитать доску.
///
/// Содержимое, которое не разбирается, возвращается как `Unreadable`, и из
/// этого состояния нет пути к записи (FR-017): повреждённый файл не
/// перезаписывается — ровно как испорченный `settings.toml` в 004.
pub async fn read_board<B: VaultBackend>(
    backend: &B,
    path: &Path,
) -> Result<BoardContent, ReadBoardError> {
    let raw = backend.read(path).await.map_err(|error| match error {
        ReadError::NotFound => ReadBoardError::NotFound,
        // Доска больше поддерживаемого размера читается тем же путём, что
        // заметка, и причина у отказа та же.
        ReadError::TooLarge => ReadBoardError::Unreadable,
        ReadError::Io => ReadBoardError::Io,
    })?;

    let content = BoardContent::new(raw.into_bytes());
    match content.digest() {
        Ok(_) => Ok(content),
        Err(DigestError::Unreadable) => Err(ReadBoardError::Unreadable),
    }
}

/// Записать доску.
///
/// Три гейта, и все три проверяются до обращения к диску
/// (contracts/board-store-trait.md § Гейты записи):
/// профиль без права записи, неразрешённый конфликт по этой доске и
/// расхождение `expected` со снимком на диске.
pub async fn write_board<B: VaultBackend>(
    backend: &mut B,
    profile: BoardWriteProfile,
    path: &Path,
    content: &BoardContent,
    expected: Option<FileStamp>,
) -> Result<(), WriteBoardError> {
    if profile == BoardWriteProfile::ViewOnly {
        return Err(WriteBoardError::ReadOnlyProfile);
    }
    // FR-014: пока конфликт не разрешён, в файл не пишется ничего — в том
    // числе принудительным сбросом дебаунса автосохранения.
    if backend.pending_conflict(path).is_some() {
        return Err(WriteBoardError::Conflicted);
    }

    let text = String::from_utf8(content.as_bytes().to_vec())
        .map_err(|_| WriteBoardError::Io)?;

    backend
        .write(path, &text, expected)
        .await
        .map_err(|error| match error {
            WriteError::Conflict => WriteBoardError::Conflicted,
            WriteError::AccessDenied => WriteBoardError::VaultNotWritable,
            WriteError::Io => WriteBoardError::Io,
        })
}
