//! Доски для рисования (фича 002).
//!
//! Граница с движком проведена по одному вопросу: проверяется ли это тестом
//! ядра без браузера (research.md §9). Классификация файлов, нормализация и
//! хэш содержимого, состояние вставок, гейты записи — здесь. Рисование,
//! сериализация модели Excalidraw и экспорт в SVG — работа движка, и ядро их
//! не воспроизводит.
//!
//! Отсюда следует форма API: содержимое доски пересекает границу
//! **непрозрачными байтами** плюс вычисленным по ним хэшем
//! (contracts/board-store-trait.md). Смена движка не меняет ни одной сигнатуры
//! этого модуля.

pub mod digest;
pub mod embed;
pub mod events;
pub mod save;
pub mod snapshot;
pub mod store;

use crate::stamp::FileStamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use digest::{digest, BoardDigest, DigestError};
pub use embed::{embed_state, embeds_of, embeds_with_state, Embed, EmbedRef, TextRange};
pub use events::{embeds_changed, BoardEvent, BoardSaveState, EmbedState};
pub use save::SaveTracker;
pub use snapshot::{
    embed_markup, embed_metadata_in_svg, plan_embed, read_metadata_from_svg,
    read_snapshot_metadata, snapshot_metadata, snapshot_path, write_snapshot, EmbedError, EmbedId,
    EmbedPlan, SnapshotError, SnapshotMetadata, SnapshotWriteError,
};
pub use store::{
    create_board, read_board, write_board, BoardWriteProfile, CreateBoardError, EMPTY_BOARD,
};

/// Доска как файл vault (data-model.md «Board»).
///
/// Форма намеренно повторяет `vault::index::Note`: дерево работает с досками и
/// заметками единообразно, а различие несёт `vault::FileKind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Board {
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub path: PathBuf,
    pub name: String,
    pub stamp: FileStamp,
    pub has_unsaved_edits: bool,
}

/// Содержимое доски для ядра — непрозрачные байты.
///
/// Тип-обёртка, а не `Vec<u8>` напрямую: она делает невозможным случайно
/// передать сюда что-то, что ядро считает разобранным, и служит напоминанием,
/// что модель движка здесь не воспроизводится (research.md §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardContent(Vec<u8>);

impl BoardContent {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Хэш нормализованного содержимого (research.md §5).
    pub fn digest(&self) -> Result<BoardDigest, DigestError> {
        digest(&self.0)
    }
}

/// Почему доска не прочитана.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ReadBoardError {
    #[error("доска не найдена")]
    NotFound,
    /// Файл есть, но не разбирается. Перезаписывать его нельзя (FR-017).
    #[error("доска не читается")]
    Unreadable,
    #[error("ошибка чтения с диска")]
    Io,
}

/// Почему запись доски отклонена.
///
/// Каждый вариант — отдельная причина, а не общий отказ: пользователь должен
/// видеть, что именно произошло (FR-010, то же требование, что FR-008 в 001).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum WriteBoardError {
    /// Внешнее изменение при несохранённых правках либо неразрешённый конфликт
    /// (FR-014). Пока он есть, в файл не пишется ничего.
    #[error("по доске есть неразрешённый конфликт")]
    Conflicted,
    /// Профиль клиента не имеет права записи доски — мобильный просмотр
    /// (FR-035). Это отсутствие пути записи, а не флаг в интерфейсе.
    #[error("на этом клиенте доска доступна только на просмотр")]
    ReadOnlyProfile,
    #[error("vault недоступен на запись")]
    VaultNotWritable,
    #[error("ошибка записи на диск")]
    Io,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hashes_through_the_same_normalization() {
        let content = BoardContent::new(br#"{"elements":[{"id":"a","version":1}]}"#.to_vec());
        let touched = BoardContent::new(br#"{"elements":[{"id":"a","version":99}]}"#.to_vec());
        assert_eq!(content.digest().unwrap(), touched.digest().unwrap());
    }

    #[test]
    fn unreadable_content_reports_a_reason_instead_of_a_hash() {
        let broken = BoardContent::new(b"{not json".to_vec());
        assert_eq!(broken.digest(), Err(DigestError::Unreadable));
    }
}
