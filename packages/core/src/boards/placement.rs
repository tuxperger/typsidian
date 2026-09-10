//! Где лежит файл вставленной доски (фича 006, contracts/asset-placement.md).
//!
//! Единственное место, знающее это правило. Ни `snapshot.rs`, ни `embed.rs`,
//! ни UI не воспроизводят его у себя (принцип I).
//!
//! Все функции здесь **чистые**: ни чтения файлов, ни записи, ни кэша. Отсюда
//! следует, что вся таблица «политика × расположение заметки × набор разделов»
//! проверяется одинаково под `cargo test` и `wasm-bindgen-test`, без файловой
//! системы. Условная компиляция в модуле запрещена.

use super::snapshot::EmbedId;
use crate::vault::sections::{nearest_section, SectionMark};
use crate::vault::{classify, FileKind, ASSETS_FOLDER_NAME};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Куда пользователь просит класть файлы вставок (data-model.md
/// «AssetsPlacement», FR-001).
///
/// Варианты различаются **только корнем**: раскладка внутри корня одна на все
/// три (FR-004). Значение принадлежит vault, а не устройству (FR-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "kebab-case")]
pub enum AssetsPlacement {
    /// Рядом с заметкой — тот же корень, что действовал до фичи 006 (FR-003).
    ///
    /// Раскладка при этом новая: одна общая папка ассетов на папку с
    /// заметками вместо отдельной папки на каждую заметку.
    #[default]
    NextToNote,
    /// В корне vault.
    VaultRoot,
    /// В корне ближайшего помеченного раздела; если помеченных предков нет —
    /// в корне vault, без ошибки (FR-006).
    SectionRoot,
}

/// Корень, в котором лежит папка ассетов для этой заметки.
///
/// Пустой путь означает корень vault.
pub fn assets_root(
    placement: AssetsPlacement,
    note_path: &Path,
    sections: &[SectionMark],
) -> PathBuf {
    match placement {
        AssetsPlacement::NextToNote => note_path.parent().unwrap_or(Path::new("")).to_path_buf(),
        AssetsPlacement::VaultRoot => PathBuf::new(),
        AssetsPlacement::SectionRoot => nearest_section(sections, note_path)
            .map(|section| section.path.clone())
            .unwrap_or_default(),
    }
}

/// Путь снимка **относительно корня vault** — им пользуется запись файла.
///
/// `None` — путь не является путём заметки. Тот же контракт, что у
/// `snapshot::assets_folder_name`, продолжающей обслуживать прежнюю раскладку.
pub fn snapshot_path(
    placement: AssetsPlacement,
    note_path: &Path,
    sections: &[SectionMark],
    embed_id: &EmbedId,
) -> Option<PathBuf> {
    let folder = note_assets_folder(placement, note_path, sections)?;
    Some(folder.join(embed_id.file_name()))
}

/// Папка снимков этой заметки относительно корня vault:
/// `<корень>/assets/<имя заметки без расширения>/`.
pub fn note_assets_folder(
    placement: AssetsPlacement,
    note_path: &Path,
    sections: &[SectionMark],
) -> Option<PathBuf> {
    if classify(note_path) != FileKind::Note {
        return None;
    }
    let stem = note_path.file_stem()?.to_str()?;
    Some(
        assets_root(placement, note_path, sections)
            .join(ASSETS_FOLDER_NAME)
            .join(stem),
    )
}

/// Путь снимка **относительно папки заметки** — он попадает в текст заметки.
///
/// Относительный, а не от корня проекта Typst, и это существенно: разбор
/// вставок (`embed::embeds_of`) чистый и корня vault не знает. Цена — цепочка
/// `../` у глубоко вложенной заметки при политике «в корне»; она читаема и
/// корректна, а абсолютный адрес стоил бы чистоты разбора (research.md §7).
///
/// Разделитель всегда `/`, включая Windows: это путь внутри документа Typst, а
/// не путь файловой системы.
pub fn markup_path(
    placement: AssetsPlacement,
    note_path: &Path,
    sections: &[SectionMark],
    embed_id: &EmbedId,
) -> Option<String> {
    let target = snapshot_path(placement, note_path, sections, embed_id)?;
    let from = note_path.parent().unwrap_or(Path::new(""));
    Some(relative_to(from, &target))
}

/// Путь до `target` из папки `from`, обе — относительно корня vault.
fn relative_to(from: &Path, target: &Path) -> String {
    let from: Vec<_> = from.components().collect();
    let target: Vec<_> = target.components().collect();
    let shared = from
        .iter()
        .zip(target.iter())
        .take_while(|(left, right)| left == right)
        .count();

    let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), from.len() - shared).collect();
    parts.extend(
        target[shared..]
            .iter()
            .map(|component| component.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Идентификатор фиксирован: расположение от него не зависит, и настоящий
    /// хэш только зашумил бы ожидаемые значения.
    fn embed_id() -> EmbedId {
        EmbedId::from_file_name("board-47dc7994.svg").expect("правильное имя снимка")
    }

    fn marks(paths: &[&str]) -> Vec<SectionMark> {
        paths
            .iter()
            .map(|path| SectionMark::new(Path::new(path)).unwrap())
            .collect()
    }

    fn path_of(placement: AssetsPlacement, note: &str, sections: &[&str]) -> Option<String> {
        snapshot_path(placement, Path::new(note), &marks(sections), &embed_id())
            .map(|path| path.to_string_lossy().replace('\\', "/"))
    }

    fn markup_of(placement: AssetsPlacement, note: &str, sections: &[&str]) -> Option<String> {
        markup_path(placement, Path::new(note), &marks(sections), &embed_id())
    }

    // Таблица contracts/asset-placement.md → «Проверяемые сценарии».

    #[test]
    fn next_to_note_puts_assets_beside_the_note() {
        assert_eq!(
            path_of(AssetsPlacement::NextToNote, "Проекты/Идея.typ", &[]).as_deref(),
            Some("Проекты/assets/Идея/board-47dc7994.svg")
        );
        assert_eq!(
            path_of(AssetsPlacement::NextToNote, "Идея.typ", &[]).as_deref(),
            Some("assets/Идея/board-47dc7994.svg")
        );
    }

    #[test]
    fn vault_root_ignores_where_the_note_lies() {
        assert_eq!(
            path_of(AssetsPlacement::VaultRoot, "Проекты/Идея.typ", &[]).as_deref(),
            Some("assets/Идея/board-47dc7994.svg")
        );
    }

    #[test]
    fn section_root_uses_the_nearest_marked_ancestor() {
        assert_eq!(
            path_of(
                AssetsPlacement::SectionRoot,
                "Работа/Проекты/Идея.typ",
                &["Работа"]
            )
            .as_deref(),
            Some("Работа/assets/Идея/board-47dc7994.svg")
        );
        assert_eq!(
            path_of(
                AssetsPlacement::SectionRoot,
                "Работа/Проекты/Идея.typ",
                &["Работа", "Работа/Проекты"]
            )
            .as_deref(),
            Some("Работа/Проекты/assets/Идея/board-47dc7994.svg")
        );
    }

    #[test]
    fn section_root_falls_back_to_the_vault_root_without_an_error() {
        // FR-006: отсутствие помеченных предков — не ошибка.
        assert_eq!(
            path_of(AssetsPlacement::SectionRoot, "Личное/Мысли.typ", &["Работа"]).as_deref(),
            Some("assets/Мысли/board-47dc7994.svg")
        );
    }

    #[test]
    fn a_section_prefix_is_matched_by_components() {
        // «Работа» не захватывает «Работа-архив».
        assert_eq!(
            path_of(
                AssetsPlacement::SectionRoot,
                "Работа-архив/Идея.typ",
                &["Работа"]
            )
            .as_deref(),
            Some("assets/Идея/board-47dc7994.svg")
        );
    }

    #[test]
    fn only_notes_get_a_placement() {
        for placement in [
            AssetsPlacement::NextToNote,
            AssetsPlacement::VaultRoot,
            AssetsPlacement::SectionRoot,
        ] {
            assert_eq!(path_of(placement, "Схема.excalidraw", &[]), None);
            assert_eq!(path_of(placement, "Фото.png", &[]), None);
        }
    }

    #[test]
    fn the_default_placement_is_next_to_the_note() {
        // FR-003: vault, в котором выбор не сделан, ведёт себя как прежде —
        // в части корня.
        assert_eq!(AssetsPlacement::default(), AssetsPlacement::NextToNote);
        assert_eq!(
            path_of(AssetsPlacement::default(), "Проекты/Идея.typ", &[]).as_deref(),
            Some("Проекты/assets/Идея/board-47dc7994.svg")
        );
    }

    #[test]
    fn markup_is_relative_to_the_note_folder() {
        assert_eq!(
            markup_of(AssetsPlacement::NextToNote, "Проекты/Идея.typ", &[]).as_deref(),
            Some("assets/Идея/board-47dc7994.svg")
        );
        assert_eq!(
            markup_of(AssetsPlacement::VaultRoot, "Проекты/Идея.typ", &[]).as_deref(),
            Some("../assets/Идея/board-47dc7994.svg")
        );
        assert_eq!(
            markup_of(AssetsPlacement::VaultRoot, "a/b/c/Идея.typ", &[]).as_deref(),
            Some("../../../assets/Идея/board-47dc7994.svg")
        );
    }

    #[test]
    fn markup_never_uses_a_backslash() {
        // Путь внутри документа Typst, а не путь файловой системы.
        let markup = markup_of(AssetsPlacement::VaultRoot, "a/b/Идея.typ", &[]).unwrap();
        assert!(!markup.contains('\\'));
    }

    #[test]
    fn the_layout_inside_the_root_is_the_same_for_every_placement() {
        // FR-004: различается только корень. Хвост у всех трёх одинаков.
        let tail = "assets/Идея/board-47dc7994.svg";
        for placement in [
            AssetsPlacement::NextToNote,
            AssetsPlacement::VaultRoot,
            AssetsPlacement::SectionRoot,
        ] {
            let path = path_of(placement, "Работа/Проекты/Идея.typ", &["Работа"]).unwrap();
            assert!(path.ends_with(tail), "{path} обязан оканчиваться на {tail}");
        }
    }
}
