//! Папка вложений и путь во вставке (FR-016 … FR-019, research.md §6, §9).
//!
//! Настройка — уровня vault: она едет вместе с ним и действует на всех
//! устройствах (FR-017). Смена ничего не перемещает: читается она только при
//! следующей вставке (FR-019).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::export::folder::ExportFolderProblem;
use crate::vault::vault_settings::VaultOverrides;

/// Папка вложений по умолчанию. Одна для всех языков интерфейса: это
/// содержимое vault, а не текст интерфейса (Assumptions спеки).
pub const DEFAULT_ATTACHMENTS_FOLDER: &str = "attachments";

/// Где лежат новые вложения (FR-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "kebab-case")]
pub enum AttachmentsPlacement {
    /// Подпапка рядом с заметкой: путь во вставке короткий и переживает перенос
    /// папки с заметкой целиком.
    #[default]
    NextToNote,
    /// Одна папка от корня vault.
    VaultFolder,
}

/// Почему название папки вложений не годится (FR-018).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum AttachmentsFolderProblem {
    #[error("папка вложений не указана")]
    Empty,
    #[error("папка вложений должна лежать внутри vault — укажите путь от его корня")]
    Absolute,
    #[error("путь к папке вложений не может содержать «..» и «.»")]
    LeavesVault,
    #[error("служебная папка приложения не может быть папкой вложений")]
    ServiceFolder,
}

impl From<ExportFolderProblem> for AttachmentsFolderProblem {
    fn from(problem: ExportFolderProblem) -> Self {
        match problem {
            ExportFolderProblem::Empty => AttachmentsFolderProblem::Empty,
            ExportFolderProblem::Absolute => AttachmentsFolderProblem::Absolute,
            ExportFolderProblem::LeavesVault => AttachmentsFolderProblem::LeavesVault,
            ExportFolderProblem::ServiceFolder => AttachmentsFolderProblem::ServiceFolder,
        }
    }
}

/// Проверяет и нормализует название папки вложений: правило то же, что у
/// папки экспорта (обязательство 1 контракта), отличается только тип причины.
pub fn validate_folder(raw: &str) -> Result<String, AttachmentsFolderProblem> {
    crate::export::folder::validate_folder(raw).map_err(AttachmentsFolderProblem::from)
}

/// Название папки вложений с учётом умолчания. Проверка повторяется: путь не
/// имеет права выйти из vault из-за набора, собранного в обход разбора.
pub fn attachments_folder(overrides: &VaultOverrides) -> String {
    overrides
        .attachments_folder
        .as_deref()
        .and_then(|raw| validate_folder(raw).ok())
        .unwrap_or_else(|| DEFAULT_ATTACHMENTS_FOLDER.to_owned())
}

/// Размещение с учётом умолчания.
pub fn placement(overrides: &VaultOverrides) -> AttachmentsPlacement {
    overrides.attachments_placement.unwrap_or_default()
}

/// Папка назначения новых вложений заметки, от корня vault (обязательство 2).
pub fn target_folder(overrides: &VaultOverrides, note_path: &Path) -> PathBuf {
    let folder = attachments_folder(overrides);
    match placement(overrides) {
        AttachmentsPlacement::NextToNote => match note_path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join(&folder),
            _ => PathBuf::from(folder),
        },
        AttachmentsPlacement::VaultFolder => PathBuf::from(folder),
    }
}

/// Путь во вставке: от папки заметки до файла, через `/`, с `../` при общей
/// папке (обязательство 3). Тот же расчёт, что у снимков досок.
pub fn markup_path(note_path: &Path, attachment_path: &Path) -> String {
    let from = note_path.parent().unwrap_or_else(|| Path::new(""));
    crate::boards::placement::relative_to(from, attachment_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overrides(placement: AttachmentsPlacement, folder: Option<&str>) -> VaultOverrides {
        VaultOverrides {
            attachments_placement: Some(placement),
            attachments_folder: folder.map(str::to_owned),
            ..VaultOverrides::default()
        }
    }

    #[test]
    fn next_to_note_by_default() {
        let defaults = VaultOverrides::default();
        assert_eq!(
            target_folder(&defaults, Path::new("Диссертация/Глава.typ")),
            PathBuf::from("Диссертация/attachments")
        );
        assert_eq!(
            target_folder(&defaults, Path::new("Корень.typ")),
            PathBuf::from("attachments")
        );
    }

    #[test]
    fn vault_folder_ignores_the_note_folder() {
        let settings = overrides(AttachmentsPlacement::VaultFolder, Some("файлы"));
        assert_eq!(
            target_folder(&settings, Path::new("a/b/c.typ")),
            PathBuf::from("файлы")
        );
    }

    #[test]
    fn markup_path_climbs_out_of_the_note_folder() {
        assert_eq!(
            markup_path(Path::new("a/b/c.typ"), Path::new("файлы/x.png")),
            "../../файлы/x.png"
        );
        assert_eq!(
            markup_path(Path::new("a/c.typ"), Path::new("a/attachments/x.png")),
            "attachments/x.png"
        );
        assert_eq!(
            markup_path(Path::new("c.typ"), Path::new("attachments/x.png")),
            "attachments/x.png"
        );
    }

    #[test]
    fn bad_folder_names_are_refused_like_the_export_folder() {
        assert_eq!(validate_folder(""), Err(AttachmentsFolderProblem::Empty));
        assert_eq!(
            validate_folder("/tmp"),
            Err(AttachmentsFolderProblem::Absolute)
        );
        assert_eq!(
            validate_folder("../x"),
            Err(AttachmentsFolderProblem::LeavesVault)
        );
        assert_eq!(
            validate_folder(".typsidian/x"),
            Err(AttachmentsFolderProblem::ServiceFolder)
        );
        assert_eq!(
            validate_folder(" файлы//картинки "),
            Ok("файлы/картинки".into())
        );
    }

    #[test]
    fn a_bad_stored_folder_falls_back_to_the_default() {
        let settings = overrides(AttachmentsPlacement::VaultFolder, Some("../вне"));
        assert_eq!(attachments_folder(&settings), DEFAULT_ATTACHMENTS_FOLDER);
    }
}
