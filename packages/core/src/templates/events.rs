//! Изменение состава библиотеки заготовок (T009, FR-003).
//!
//! # Своего вотчера здесь нет и не будет
//!
//! Галерея слушает те же `VaultEvent`, что и дерево заметок (research.md §3).
//! Второй источник событий по тем же файлам означал бы две очереди с разным
//! порядком доставки и гонки между ними — а гарантия «внешнее изменение видно
//! ≤2 с» уже дана фичей 001 и покрыта её тестами на обоих таргетах.
//!
//! Роль этого модуля поэтому скромная и ровно одна: **перевести** событие о
//! файле в ответ на вопрос «изменился ли состав библиотеки». Вопрос неочевидный
//! ровно настолько, чтобы не решать его в интерфейсе: правку заготовки от
//! появления новой отличает не тип события, а то, куда указывает путь.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::vault::events::VaultEvent;
use crate::vault::{classify, FileKind};

use super::library::is_inside_templates_folder;

/// Что случилось с библиотекой заготовок.
///
/// Пути наружу не выносятся: галерея на любое из этих событий перечитывает
/// состав целиком у ядра. Перечитывание дешёвое — это проекция уже
/// построенного индекса, — а инкрементальное обновление списка в интерфейсе
/// было бы вторым местом, знающим состав библиотеки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "type")]
pub enum TemplateEvent {
    /// Состав библиотеки изменился: заготовка появилась, исчезла или переехала.
    LibraryChanged,
    /// Содержимое заготовки изменилось — превью для неё устарело.
    ///
    /// Отдельно от [`Self::LibraryChanged`] потому, что это разные действия
    /// галереи: список перестраивать не нужно, пересчитать нужно превью, и
    /// только для изменившейся карточки (SC-010).
    TemplateChanged,
}

/// Затрагивает ли событие файла библиотеку заготовок (FR-003).
///
/// `None` — событие о библиотеке ничего не говорит: файл лежит вне
/// `templates/`, или это вообще не заметка.
pub fn library_event(event: &VaultEvent) -> Option<TemplateEvent> {
    match event {
        VaultEvent::NoteCreated { path } | VaultEvent::NoteDeleted { path } => {
            is_template(path).then_some(TemplateEvent::LibraryChanged)
        }
        VaultEvent::NoteDeletedWithUnsavedEdits { note_path } => {
            is_template(note_path).then_some(TemplateEvent::LibraryChanged)
        }
        // Переезд может и вносить заготовку в библиотеку, и выносить из неё —
        // достаточно, чтобы заготовкой был любой из двух путей.
        VaultEvent::NoteRenamed { from_path, to_path } => {
            (is_template(from_path) || is_template(to_path)).then_some(TemplateEvent::LibraryChanged)
        }
        VaultEvent::NoteUpdated { path } => {
            is_template(path).then_some(TemplateEvent::TemplateChanged)
        }
        VaultEvent::ConflictDetected { note_path, .. } => {
            is_template(note_path).then_some(TemplateEvent::TemplateChanged)
        }
        // Массовое изменение и потеря доступа касаются библиотеки так же, как
        // всего остального: состав перечитывается вместе с деревом.
        VaultEvent::BulkChangeSettled | VaultEvent::VaultAccessChanged { .. } => {
            Some(TemplateEvent::LibraryChanged)
        }
        VaultEvent::BulkChangeStarted
        | VaultEvent::LinksInvalidated { .. }
        | VaultEvent::VaultSettingsLost
        | VaultEvent::VaultSettingsReadOnly => None,
    }
}

/// Заготовка ли это: заметка внутри папки заготовок.
fn is_template(path: &Path) -> bool {
    is_inside_templates_folder(path) && classify(path) == FileKind::Note
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_note_outside_the_templates_folder_says_nothing_about_the_library() {
        assert_eq!(
            library_event(&VaultEvent::NoteCreated {
                path: PathBuf::from("Диссертация/Введение.typ")
            }),
            None
        );
    }

    #[test]
    fn a_new_file_in_the_templates_folder_changes_the_library() {
        assert_eq!(
            library_event(&VaultEvent::NoteCreated {
                path: PathBuf::from("templates/раздел.typ")
            }),
            Some(TemplateEvent::LibraryChanged)
        );
    }

    #[test]
    fn editing_a_template_invalidates_its_preview_but_not_the_list() {
        assert_eq!(
            library_event(&VaultEvent::NoteUpdated {
                path: PathBuf::from("templates/раздел.typ")
            }),
            Some(TemplateEvent::TemplateChanged)
        );
    }

    #[test]
    fn moving_a_note_into_the_folder_counts_as_a_library_change() {
        assert_eq!(
            library_event(&VaultEvent::NoteRenamed {
                from_path: PathBuf::from("Черновики/раздел.typ"),
                to_path: PathBuf::from("templates/раздел.typ"),
            }),
            Some(TemplateEvent::LibraryChanged)
        );
    }

    #[test]
    fn a_board_in_the_templates_folder_is_not_a_template() {
        // Содержимое доски для ядра — непрозрачные байты (фича 002): вставить
        // его в текст заметки нельзя, значит и заготовкой оно не является.
        assert_eq!(
            library_event(&VaultEvent::NoteCreated {
                path: PathBuf::from("templates/схема.excalidraw")
            }),
            None
        );
    }
}
