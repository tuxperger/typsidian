//! Состав библиотеки заготовок и состояние её папки (фича 003, T004, T006).
//!
//! Библиотека — **проекция уже построенного индекса заметок** на папку
//! `templates/`, а не отдельный обход диска (research.md §3,
//! contracts/template-library-trait.md). Второй обход означал бы второй момент
//! времени: дерево и галерея показывали бы разное состояние одних и тех же
//! файлов, и объяснить пользователю расхождение было бы нечем.
//!
//! Отсюда же два следствия спеки, за которые здесь не написано ни строчки:
//! FR-007 (заметка попадает в галерею независимо от того, как она оказалась в
//! папке) и FR-008 (заготовки участвуют в поиске и графе ссылок) — они
//! выполняются тем, что заготовка остаётся обычной заметкой индекса.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::vault::backend::VaultBackend;
use crate::vault::{AccessState, FileKind, FolderAccess, FolderProblem};

use super::Template;

/// Папка заготовок в корне vault (research.md §2).
///
/// Имя фиксировано и настройкой не является. Латиница — ради переносимости:
/// папка едет вместе с vault между ОС и файловыми системами.
pub const TEMPLATES_FOLDER: &str = "templates";

/// Путь папки заготовок относительно корня vault.
pub fn templates_folder() -> PathBuf {
    PathBuf::from(TEMPLATES_FOLDER)
}

/// Состояние папки заготовок (data-model.md «TemplateFolderState»).
///
/// `Absent` отделён от `Unreadable` намеренно: «папки ещё нет» — нормальное
/// состояние нового vault, которое чинится первым же сохранением (FR-002), а
/// «папка есть, но не читается» — проблема, о которой нужно сказать (FR-004,
/// FR-009, FR-016). Схлопывание этих двух состояний превратило бы штатный путь
/// в ошибку.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "state")]
pub enum TemplateFolderState {
    /// Папка существует, читается и принимает запись — обычная работа.
    Present,
    /// Папки нет. Пустая галерея как штатное состояние, а не ошибка.
    Absent,
    /// Папка есть, но недоступна на чтение: сообщение с причиной, остальное
    /// приложение продолжает работать.
    Unreadable { reason: FolderProblem },
    /// Чтение есть, записи нет: галерея работает, сохранение заготовки
    /// отклоняется с названной причиной (US2 AS6).
    NotWritable { reason: FolderProblem },
}

impl TemplateFolderState {
    /// Можно ли положить в папку новую заготовку.
    ///
    /// `Absent` — можно: папка создаётся первым же сохранением (FR-002).
    pub fn accepts_writes(self) -> bool {
        matches!(self, Self::Present | Self::Absent)
    }
}

/// Состав библиотеки (data-model.md «TemplateLibrary»).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TemplateLibraryView {
    /// Все заметки внутри `templates/`, включая вложенные папки, в порядке
    /// пути — том же, в котором их отдаёт индекс.
    pub templates: Vec<Template>,
    pub folder_state: TemplateFolderState,
}

/// Состояние папки заготовок (FR-004, FR-009).
///
/// Складывает два факта: состояние самой папки (его знает backend) и
/// доступность vault целиком (её знает он же, но другим вопросом). Право
/// записи, отозванное на весь vault, — это `NotWritable` у папки: галерея
/// продолжает работать, а сохранение обязано быть отклонено с причиной, и
/// причина здесь именно «vault только на чтение», а не «папка защищена».
pub async fn folder_state<B: VaultBackend>(backend: &B) -> TemplateFolderState {
    // Vault пропал целиком — про папку в нём говорить уже нечего.
    if backend.access_state() == AccessState::Unavailable {
        return TemplateFolderState::Unreadable {
            reason: FolderProblem::AccessDenied,
        };
    }

    match backend.folder_access(&templates_folder()).await {
        FolderAccess::Absent => TemplateFolderState::Absent,
        FolderAccess::Unreadable { reason } => TemplateFolderState::Unreadable { reason },
        FolderAccess::NotWritable { reason } => TemplateFolderState::NotWritable { reason },
        FolderAccess::Present if backend.access_state() == AccessState::ReadWrite => {
            TemplateFolderState::Present
        }
        FolderAccess::Present => TemplateFolderState::NotWritable {
            reason: FolderProblem::VaultReadOnly,
        },
    }
}

/// Состав библиотеки: проекция индекса заметок на папку `templates/`.
///
/// Отдельного обхода диска здесь нет и быть не должно (FR-007, research.md §3).
/// Доски и служебные файлы в библиотеку не попадают: заготовка применяется как
/// текст — вставкой в курсор или содержимым новой заметки, — а содержимое доски
/// для ядра непрозрачные байты (фича 002).
pub async fn library<B: VaultBackend>(backend: &B) -> TemplateLibraryView {
    let folder_state = folder_state(backend).await;
    let templates = backend
        .list()
        .await
        .into_iter()
        .filter(|note| note.kind == FileKind::Note)
        .filter_map(|note| Template::from_note(&note))
        .collect();

    TemplateLibraryView {
        templates,
        folder_state,
    }
}

/// Вложенная папка внутри `templates/` — группировка галереи (research.md §2).
///
/// `None` — заготовка лежит прямо в `templates/`. Путь **не** внутри папки
/// заготовок даёт `None` тоже, но такой путь сюда не доходит: его отсеивает
/// [`is_inside_templates_folder`].
pub fn group_of(path: &Path) -> Option<String> {
    let inside = path.strip_prefix(TEMPLATES_FOLDER).ok()?;
    let folder = inside.parent()?;
    if folder.as_os_str().is_empty() {
        return None;
    }
    Some(folder.to_string_lossy().into_owned())
}

/// Лежит ли файл внутри папки заготовок.
///
/// Проверяется префикс пути, а не имя папки где-нибудь в середине: заметка
/// `курс/templates/лекция.typ` заготовкой не является — папка заготовок одна и
/// она в корне vault.
pub fn is_inside_templates_folder(path: &Path) -> bool {
    path.strip_prefix(TEMPLATES_FOLDER)
        .is_ok_and(|rest| !rest.as_os_str().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nested_folder_becomes_a_group() {
        assert_eq!(
            group_of(Path::new("templates/формулы/выключная.typ")),
            Some("формулы".to_owned())
        );
        assert_eq!(
            group_of(Path::new("templates/а/б/в.typ")),
            Some("а/б".to_owned())
        );
        assert_eq!(group_of(Path::new("templates/раздел.typ")), None);
    }

    #[test]
    fn the_templates_folder_is_the_one_in_the_vault_root() {
        assert!(is_inside_templates_folder(Path::new(
            "templates/раздел.typ"
        )));
        assert!(!is_inside_templates_folder(Path::new(
            "курс/templates/лекция.typ"
        )));
        // Сама папка файлом не является.
        assert!(!is_inside_templates_folder(Path::new("templates")));
    }
}
