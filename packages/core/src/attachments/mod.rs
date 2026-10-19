//! Вложения из буфера и перетаскиванием (фича 016).
//!
//! Картинка попадает в заметку одним действием: файл ложится в папку вложений
//! (рядом с заметкой либо одна папка vault — настройка vault), в текст встаёт
//! `#image("…")` с путём от заметки. Правила — вид файла по содержимому, имя,
//! папка и путь, сопоставление по байтам, запрет перезаписи — здесь и одни для
//! всех оболочек (принципы I и II); UI только собирает файлы и гоняет пачку.
//!
//! Запись — через `VaultBackend::write_bytes`, атомарную на обоих таргетах
//! (009 research.md §4): отказ не оставляет полузаписанного файла (FR-014).
//! Сети модуль не касается вовсе (research.md §15).

pub mod batch;
pub mod folder;
pub mod kind;
pub mod markup;
pub mod name;
pub mod store;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use batch::{
    AttachmentBatchPhase, AttachmentBatchState, AttachmentBatchStep, AttachmentBatchSummary,
    AttachmentBatchView, AttachmentRefusalReason, BatchItem, IncomingItem, ItemStatus, NextAction,
    Origin, StoreProblem,
};
pub use folder::{
    attachments_folder, markup_path, placement, target_folder, validate_folder,
    AttachmentsFolderProblem, AttachmentsPlacement, DEFAULT_ATTACHMENTS_FOLDER,
};
pub use kind::{sniff, ImageKind};
pub use markup::{image_markup, plan_markup};
pub use name::{free_name, generated_name, kept_name, InsertMoment};

/// Порог подтверждения большого изображения (FR-013): 20 МБ, строго больше.
pub const LARGE_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
