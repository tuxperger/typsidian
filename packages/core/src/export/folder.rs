//! Папка экспорта и путь результата (009 FR-027 … FR-029, research.md §7).
//!
//! Папка — настройка уровня vault: она принадлежит набору заметок, а не
//! устройству, и одинакова на всех машинах, открывающих этот vault (FR-028).

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::vault::service_dir;
use crate::vault::vault_settings::VaultOverrides;

/// Папка экспорта по умолчанию (FR-028).
pub const DEFAULT_EXPORT_FOLDER: &str = "export";

/// Почему папка экспорта не годится (FR-029).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ExportFolderProblem {
    /// Пусто: писать PDF в корень vault вперемешку с заметками — не папка
    /// экспорта, а её отсутствие.
    #[error("папка экспорта не указана")]
    Empty,
    /// Абсолютный путь или путь с диском: папка обязана лежать внутри vault.
    #[error("папка экспорта должна лежать внутри vault — укажите путь от его корня")]
    Absolute,
    /// `..` или `.` среди частей пути: такой путь может увести за пределы vault
    /// или означать не то, что написано.
    #[error("путь к папке экспорта не может содержать «..» и «.»")]
    LeavesVault,
    /// Служебная папка приложения: её содержимое не показывается и не
    /// принадлежит пользователю.
    #[error("служебную папку приложения нельзя сделать папкой экспорта")]
    ServiceFolder,
}

/// Проверяет и нормализует значение папки экспорта.
///
/// Нормальная форма — части через `/`, без ведущего и завершающего `/`:
/// `" /Выгрузка//2026/ "` → `"Выгрузка/2026"`. Хранится именно она, чтобы файл
/// настроек не зависел от разделителя платформы, на которой его записали.
pub fn validate_folder(raw: &str) -> Result<String, ExportFolderProblem> {
    let trimmed = raw.trim();
    if trimmed.starts_with('/') || trimmed.starts_with('\\') || has_drive_prefix(trimmed) {
        return Err(ExportFolderProblem::Absolute);
    }
    let parts: Vec<&str> = trimmed
        .split(['/', '\\'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return Err(ExportFolderProblem::Empty);
    }
    if parts.iter().any(|part| *part == ".." || *part == ".") {
        return Err(ExportFolderProblem::LeavesVault);
    }
    let normalized = parts.join("/");
    if service_dir::is_inside_service_dir(Path::new(&normalized)) {
        return Err(ExportFolderProblem::ServiceFolder);
    }
    Ok(normalized)
}

/// `C:` и подобное в начале: на Windows это абсолютный путь, даже без `\`.
fn has_drive_prefix(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(letter), Some(':')) if letter.is_ascii_alphabetic()
    )
}

/// Папка экспорта с учётом умолчания.
///
/// Негодное значение из файла здесь уже невозможно: разбор настроек
/// отбрасывает его (как непригодную пометку раздела), а запись не пропускает.
/// Проверка всё равно повторяется — путь результата не имеет права выйти из
/// vault из-за того, что кто-то собрал `VaultOverrides` в обход разбора.
pub fn export_folder(overrides: &VaultOverrides) -> String {
    overrides
        .export_folder
        .as_deref()
        .and_then(|raw| validate_folder(raw).ok())
        .unwrap_or_else(|| DEFAULT_EXPORT_FOLDER.to_owned())
}

/// Путь PDF заметки внутри vault (FR-027): путь заметки от корня vault с
/// заменой расширения, внутри папки экспорта.
pub fn pdf_path(note_path: &Path, folder: &str) -> PathBuf {
    let mut path = PathBuf::from(folder);
    for component in note_path.components() {
        if let Component::Normal(part) = component {
            path.push(part);
        }
    }
    path.set_extension("pdf");
    path
}

/// Лежит ли путь внутри папки экспорта (FR-011).
pub fn is_inside_folder(path: &Path, folder: &str) -> bool {
    path.starts_with(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_separators_and_spaces() {
        assert_eq!(validate_folder("export"), Ok("export".into()));
        assert_eq!(
            validate_folder(" Выгрузка//2026/ "),
            Ok("Выгрузка/2026".into())
        );
        assert_eq!(validate_folder("pdf\\2026"), Ok("pdf/2026".into()));
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(validate_folder(""), Err(ExportFolderProblem::Empty));
        assert_eq!(validate_folder("   "), Err(ExportFolderProblem::Empty));
    }

    #[test]
    fn rejects_absolute() {
        assert_eq!(validate_folder("/tmp"), Err(ExportFolderProblem::Absolute));
        assert_eq!(
            validate_folder("\\\\server"),
            Err(ExportFolderProblem::Absolute)
        );
        assert_eq!(
            validate_folder("C:\\pdf"),
            Err(ExportFolderProblem::Absolute)
        );
        assert_eq!(validate_folder("c:pdf"), Err(ExportFolderProblem::Absolute));
    }

    #[test]
    fn rejects_leaving_the_vault() {
        assert_eq!(
            validate_folder("../x"),
            Err(ExportFolderProblem::LeavesVault)
        );
        assert_eq!(
            validate_folder("a/../../x"),
            Err(ExportFolderProblem::LeavesVault)
        );
        assert_eq!(
            validate_folder("./x"),
            Err(ExportFolderProblem::LeavesVault)
        );
    }

    #[test]
    fn rejects_service_folder() {
        assert_eq!(
            validate_folder(".typsidian"),
            Err(ExportFolderProblem::ServiceFolder)
        );
        assert_eq!(
            validate_folder(".typsidian/pdf"),
            Err(ExportFolderProblem::ServiceFolder)
        );
        // Похожее имя — не служебная папка.
        assert_eq!(validate_folder(".typsidian2"), Ok(".typsidian2".into()));
    }

    #[test]
    fn default_and_invalid_value() {
        let mut overrides = VaultOverrides::default();
        assert_eq!(export_folder(&overrides), "export");
        overrides.export_folder = Some("pdf/".into());
        assert_eq!(export_folder(&overrides), "pdf");
        overrides.export_folder = Some("../вне".into());
        assert_eq!(export_folder(&overrides), "export");
    }

    #[test]
    fn pdf_path_keeps_note_folders() {
        assert_eq!(
            pdf_path(Path::new("Диссертация/Введение.typ"), "export"),
            PathBuf::from("export/Диссертация/Введение.pdf")
        );
        assert_eq!(
            pdf_path(Path::new("Идея.typ"), "Выгрузка/2026"),
            PathBuf::from("Выгрузка/2026/Идея.pdf")
        );
        // Точка в имени заметки не теряется: меняется только последнее расширение.
        assert_eq!(
            pdf_path(Path::new("v1.2 отчёт.typ"), "export"),
            PathBuf::from("export/v1.2 отчёт.pdf")
        );
    }

    #[test]
    fn inside_export_folder() {
        assert!(is_inside_folder(Path::new("export/a.typ"), "export"));
        assert!(!is_inside_folder(Path::new("exported/a.typ"), "export"));
        assert!(is_inside_folder(Path::new("pdf/2026/a.typ"), "pdf/2026"));
    }
}
