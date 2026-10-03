//! Что предлагать в Центре экспорта (009 FR-010, FR-011, research.md §8).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::folder::{export_folder, is_inside_folder};
use crate::vault::index::Note;
use crate::vault::vault_settings::VaultOverrides;
use crate::vault::FileKind;

/// Строка Центра экспорта.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ExportCandidate {
    pub path: String,
    /// Родительская папка через `/`; `""` — корень vault. По ней UI группирует
    /// строки так же, как дерево.
    pub folder: String,
    pub name: String,
    pub exportable: bool,
    pub reason: Option<NotExportable>,
}

/// Почему строку нельзя отметить.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum NotExportable {
    /// Доска: в PDF попадает снимком внутри заметки, а не сама по себе.
    Board,
    /// Заметка лежит в папке экспорта: её экспорт вкладывал бы результаты
    /// друг в друга (FR-011).
    InsideExportFolder,
}

/// Строки Центра экспорта из тех же записей, что показывает дерево.
///
/// Порядок — корень vault, затем папки по алфавиту, внутри — по имени:
/// диапазон Shift-выбора (FR-012) строится именно в нём. Служебные файлы в дерево не попадают и здесь не появятся;
/// всё, что не заметка и не доска, отбрасывается на всякий случай.
pub fn candidates(notes: &[Note], overrides: &VaultOverrides) -> Vec<ExportCandidate> {
    let folder = export_folder(overrides);
    let mut rows: Vec<ExportCandidate> = notes
        .iter()
        .filter_map(|note| {
            let reason = match note.kind {
                FileKind::Note if is_inside_folder(&note.path, &folder) => {
                    Some(NotExportable::InsideExportFolder)
                }
                FileKind::Note => None,
                FileKind::Board => Some(NotExportable::Board),
                _ => return None,
            };
            Some(ExportCandidate {
                path: slash(&note.path),
                folder: note.path.parent().map(slash).unwrap_or_default(),
                name: note.name.clone(),
                exportable: reason.is_none(),
                reason,
            })
        })
        .collect();
    // По папке, затем по имени: строки одной папки идут подряд, и порядок
    // списка совпадает с тем, что UI показывает группами. Иначе диапазон
    // Shift-выбора (FR-012) захватывал бы строки, которых на экране между
    // двумя отмеченными нет.
    rows.sort_by(|a, b| (&a.folder, &a.path).cmp(&(&b.folder, &b.path)));
    rows
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::index::FileStamp;
    use std::path::PathBuf;

    fn note(path: &str) -> Note {
        let stamp = FileStamp {
            mtime_millis: 0,
            size: 0,
            content_hash: [0; 32],
        };
        Note::new(PathBuf::from(path), stamp)
    }

    #[test]
    fn notes_are_exportable_boards_are_not() {
        let rows = candidates(
            &[
                note("Б.typ"),
                note("Схема.excalidraw"),
                note("Диссертация/А.typ"),
            ],
            &VaultOverrides::default(),
        );
        let summary: Vec<(&str, bool, Option<NotExportable>)> = rows
            .iter()
            .map(|row| (row.path.as_str(), row.exportable, row.reason))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("Б.typ", true, None),
                ("Схема.excalidraw", false, Some(NotExportable::Board)),
                ("Диссертация/А.typ", true, None),
            ]
        );
        assert_eq!(rows[0].folder, "");
        assert_eq!(rows[2].folder, "Диссертация");
    }

    #[test]
    fn notes_inside_the_export_folder_are_not_offered() {
        let mut overrides = VaultOverrides::default();
        overrides.export_folder = Some("pdf".into());
        let rows = candidates(&[note("pdf/Копия.typ"), note("export/А.typ")], &overrides);
        assert_eq!(rows[0].path, "export/А.typ");
        assert!(rows[0].exportable);
        assert_eq!(rows[1].reason, Some(NotExportable::InsideExportFolder));
    }
}
