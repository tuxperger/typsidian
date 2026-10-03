//! Разделы vault — папки, помеченные пользователем как корни для файлов
//! вставок (фича 006, contracts/vault-settings.md → «Разделы»).
//!
//! Пометка хранится **путём относительно корня vault**, а не файлом-маркером
//! внутри папки. Маркер нарушил бы FR-035 (никаких служебных файлов-меток
//! внутри vault) и разложил бы служебные файлы по всему дереву — ровно тот
//! беспорядок, против которого затевалась фича.
//!
//! Отсюда же следствие: переименование папки **средствами приложения** пометку
//! переносит (FR-018) — приложение об этом переименовании знает, и терять
//! пометку было бы просто дефектом, — а переименование снаружи пометку теряет
//! (FR-019). Розыска по новому расположению нет: то же правило, что для
//! записей vault в `settings::folder_ref`.

use std::path::{Component, Path, PathBuf};

/// Папка, помеченная как корень раздела (data-model.md «SectionMark»).
///
/// Пустой путь — помечен сам корень vault: допустимо, и тогда вариант
/// «в корне раздела» совпадает с вариантом «в корне».
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SectionMark {
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub path: PathBuf,
}

impl SectionMark {
    /// Создаёт пометку, нормализуя путь.
    ///
    /// `None` — путь непригоден: абсолютный либо выходящий за корень через
    /// `..`. Хранить такое нельзя: пометка обязана указывать внутрь vault
    /// (принцип VI), а проверять это в каждом месте использования — значит
    /// однажды забыть.
    pub fn new(path: &Path) -> Option<Self> {
        Some(Self {
            path: normalize(path)?,
        })
    }
}

/// Приводит путь к виду, в котором пометки сравнимы между собой: без `./`,
/// без хвостового разделителя, только обычные компоненты.
///
/// `None` — путь абсолютный или содержит `..`.
pub fn normalize(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => normalized.push(name),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(normalized)
}

/// Ближайший помеченный предок заметки (FR-016).
///
/// «Ближайший» — самый длинный путь из `sections`, являющийся префиксом пути
/// заметки. Сравнение идёт **по компонентам**, а не по строке: иначе раздел
/// `Работа` захватил бы заметки из `Работа-архив`, которая ему не подчинена.
///
/// `None` — помеченных предков нет; вызывающая сторона обязана взять корень
/// vault, и это не ошибка (FR-006).
pub fn nearest_section<'a>(
    sections: &'a [SectionMark],
    note_path: &Path,
) -> Option<&'a SectionMark> {
    let folder = note_path.parent().unwrap_or(Path::new(""));
    sections
        .iter()
        .filter(|section| is_ancestor_or_self(&section.path, folder))
        .max_by_key(|section| section.path.components().count())
}

/// Является ли `candidate` папкой-предком `folder` или самой этой папкой.
fn is_ancestor_or_self(candidate: &Path, folder: &Path) -> bool {
    let mut wanted = candidate.components();
    let mut actual = folder.components();
    loop {
        match (wanted.next(), actual.next()) {
            // Кандидат кончился — значит был префиксом (в том числе пустой
            // путь, то есть помеченный корень vault).
            (None, _) => return true,
            (Some(_), None) => return false,
            (Some(left), Some(right)) if left != right => return false,
            (Some(_), Some(_)) => {}
        }
    }
}

/// Переносит пометки вслед за переименованием или переносом папки средствами
/// приложения (FR-018).
///
/// Переезжает не только сама помеченная папка, но и все пометки под ней:
/// переименование `Работа` обязано увезти с собой `Работа/Проекты`, иначе
/// вложенный раздел молча потерял бы адресата, хотя его папка на месте.
///
/// Возвращает `true`, если хоть одна пометка изменилась, — вызывающая сторона
/// по этому признаку решает, нужна ли запись файла настроек.
pub fn rename_section(sections: &mut [SectionMark], from: &Path, to: &Path) -> bool {
    let (Some(from), Some(to)) = (normalize(from), normalize(to)) else {
        return false;
    };
    let mut changed = false;
    for section in sections.iter_mut() {
        if !is_ancestor_or_self(&from, &section.path) {
            continue;
        }
        let tail: PathBuf = section
            .path
            .components()
            .skip(from.components().count())
            .collect();
        let moved = to.join(tail);
        if moved != section.path {
            section.path = moved;
            changed = true;
        }
    }
    changed
}

/// Пометки, чьи папки больше не существуют (FR-019).
///
/// **Только сообщает.** Молчаливое удаление скрыло бы от пользователя, что
/// пометка потеряна, и новые вставки начали бы уезжать в корень без всякого
/// объяснения. Что делать с результатом — решает вызывающая сторона.
pub fn prune_missing(sections: &[SectionMark], existing_folders: &[PathBuf]) -> Vec<SectionMark> {
    sections
        .iter()
        .filter(|section| {
            // Помеченный корень vault существует всегда.
            !section.path.as_os_str().is_empty()
                && !existing_folders
                    .iter()
                    .any(|folder| normalize(folder).is_some_and(|folder| folder == section.path))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(paths: &[&str]) -> Vec<SectionMark> {
        paths
            .iter()
            .map(|path| SectionMark::new(Path::new(path)).unwrap())
            .collect()
    }

    #[test]
    fn the_nearest_marked_ancestor_wins() {
        // FR-016: пометка вложенной папки перекрывает более верхнюю.
        let sections = marks(&["Работа", "Работа/Проекты"]);
        let found = nearest_section(&sections, Path::new("Работа/Проекты/Идея.typ")).unwrap();
        assert_eq!(found.path, Path::new("Работа/Проекты"));
    }

    #[test]
    fn a_prefix_is_matched_by_components_not_by_string() {
        // «Работа» не является разделом для «Работа-архив»: сравнение по
        // строке дало бы здесь ложное совпадение.
        let sections = marks(&["Работа"]);
        assert!(nearest_section(&sections, Path::new("Работа-архив/Идея.typ")).is_none());
    }

    #[test]
    fn without_marked_ancestors_there_is_no_section() {
        let sections = marks(&["Работа"]);
        assert!(nearest_section(&sections, Path::new("Личное/Мысли.typ")).is_none());
    }

    #[test]
    fn the_vault_root_can_be_marked() {
        let sections = marks(&[""]);
        let found = nearest_section(&sections, Path::new("Личное/Мысли.typ")).unwrap();
        assert_eq!(found.path, Path::new(""));
    }

    #[test]
    fn a_note_in_a_marked_folder_itself_belongs_to_it() {
        let sections = marks(&["Работа"]);
        let found = nearest_section(&sections, Path::new("Работа/Идея.typ")).unwrap();
        assert_eq!(found.path, Path::new("Работа"));
    }

    #[test]
    fn renaming_carries_the_mark_and_everything_under_it() {
        // FR-018: приложение знает об этом переименовании, терять пометку
        // было бы дефектом.
        let mut sections = marks(&["Работа", "Работа/Проекты", "Личное"]);
        assert!(rename_section(
            &mut sections,
            Path::new("Работа"),
            Path::new("Служба")
        ));
        assert_eq!(sections[0].path, Path::new("Служба"));
        assert_eq!(sections[1].path, Path::new("Служба/Проекты"));
        // Соседняя ветка не тронута.
        assert_eq!(sections[2].path, Path::new("Личное"));
    }

    #[test]
    fn renaming_an_unmarked_folder_changes_nothing() {
        let mut sections = marks(&["Работа"]);
        assert!(!rename_section(
            &mut sections,
            Path::new("Личное"),
            Path::new("Личное-2")
        ));
        assert_eq!(sections[0].path, Path::new("Работа"));
    }

    #[test]
    fn missing_folders_are_reported_not_removed() {
        // FR-019: розыска нет, удаления тоже — только факт.
        let sections = marks(&["Работа", "Личное"]);
        let lost = prune_missing(&sections, &[PathBuf::from("Работа")]);
        assert_eq!(lost, marks(&["Личное"]));
        // Исходный список не изменился: функция ничего не удаляет.
        assert_eq!(sections.len(), 2);
    }

    #[test]
    fn a_mark_cannot_point_outside_the_vault() {
        assert!(SectionMark::new(Path::new("../снаружи")).is_none());
        assert!(SectionMark::new(Path::new("Работа/../..")).is_none());
        assert_eq!(
            SectionMark::new(Path::new("./Работа/")).unwrap().path,
            Path::new("Работа")
        );
    }
}
