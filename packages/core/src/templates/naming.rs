//! Имя будущей заготовки: допустимость и коллизия (T031).
//!
//! Валидация имени **переиспользуется** из vault-manager
//! ([`is_valid_note_name`]), а не пишется здесь заново (research.md §7, §8).
//! Второе правило о допустимых именах было бы вторым источником истины ровно
//! там, где фича 001 его устранила: заготовка — обычная заметка, и набор имён,
//! которые ей разрешены, обязан совпадать до символа.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::vault::backend::{is_valid_note_name, VaultBackend};
use crate::vault::NOTE_EXTENSION;

use super::library::templates_folder;

/// Почему имя не годится (FR-029).
///
/// Отдельные варианты, а не текст отказа: причину показывает интерфейс на своём
/// языке, а решение «пустить дальше или нет» принимается по варианту.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum NameProblem {
    /// Имя пустое или состоит из одних пробелов.
    Empty,
    /// В имени есть разделитель пути или NUL — файловая система такого имени
    /// не примет (правило 001 FR-013).
    IllegalCharacters,
}

/// Результат проверки имени **до** любых изменений на диске (FR-029).
///
/// `NameTaken` — отдельный вариант, а не разновидность `Invalid`: имя
/// корректно, вопрос только в подтверждении перезаписи (FR-028).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "check")]
pub enum NameCheck {
    /// Имя допустимо и свободно — можно сохранять.
    Ok,
    /// Имя недопустимо для файловой системы: объяснить причину, на диск не
    /// ходить.
    Invalid { reason: NameProblem },
    /// Заготовка с таким именем уже есть: спросить подтверждение перезаписи.
    NameTaken,
}

/// Имя файла заготовки: то, что ввёл пользователь, плюс расширение заметки.
///
/// Расширение добавляется, если его нет: пользователь вводит «Раздел
/// диссертации», а не «Раздел диссертации.typ». Уже написанное расширение
/// повторно не приписывается — иначе получился бы `раздел.typ.typ`.
pub fn template_file_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    let with_extension = if Path::new(trimmed)
        .extension()
        .and_then(|extension| extension.to_str())
        == Some(NOTE_EXTENSION)
    {
        trimmed.to_owned()
    } else {
        format!("{trimmed}.{NOTE_EXTENSION}")
    };
    // Проверка **после** приписывания расширения: имя «а/б» недопустимо и в
    // виде «а/б.typ», а имя, ставшее недопустимым только из-за расширения,
    // существовать не может — расширение состоит из допустимых символов.
    is_valid_note_name(&with_extension).then_some(with_extension)
}

/// Полный путь заготовки внутри vault, если имя допустимо.
pub fn template_path(name: &str) -> Result<PathBuf, NameProblem> {
    if name.trim().is_empty() {
        return Err(NameProblem::Empty);
    }
    template_file_name(name)
        .map(|file| templates_folder().join(file))
        .ok_or(NameProblem::IllegalCharacters)
}

/// Проверяет имя будущей заготовки, ничего не меняя на диске (FR-029).
///
/// Коллизия ищется по индексу заметок — тому же, из которого строится
/// библиотека. Отдельного обращения к диску здесь нет: сам факт «файл с таким
/// путём существует» индекс уже знает.
///
/// Гарантии «между проверкой и записью ничего не изменится» эта функция **не
/// даёт и дать не может**. Её и не нужно: то же правило продублировано в
/// [`super::save_as_template`], которое без `confirm_overwrite` откажет на
/// занятом имени, даже если интерфейс проверку пропустил
/// (contracts/template-library-trait.md).
pub async fn check_name<B: VaultBackend>(backend: &B, name: &str) -> NameCheck {
    let path = match template_path(name) {
        Ok(path) => path,
        Err(reason) => return NameCheck::Invalid { reason },
    };

    if exists(backend, &path).await {
        NameCheck::NameTaken
    } else {
        NameCheck::Ok
    }
}

/// Есть ли уже файл по этому пути, по данным индекса.
pub(super) async fn exists<B: VaultBackend>(backend: &B, path: &Path) -> bool {
    backend
        .list()
        .await
        .iter()
        .any(|note| note.path.as_path() == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_note_extension_is_appended_once() {
        assert_eq!(
            template_file_name("Раздел диссертации").as_deref(),
            Some("Раздел диссертации.typ")
        );
        assert_eq!(
            template_file_name("раздел.typ").as_deref(),
            Some("раздел.typ")
        );
    }

    #[test]
    fn a_path_separator_is_not_a_name() {
        assert_eq!(
            template_path("папка/заготовка"),
            Err(NameProblem::IllegalCharacters)
        );
        assert_eq!(template_path("   "), Err(NameProblem::Empty));
    }

    #[test]
    fn a_valid_name_lands_inside_the_templates_folder() {
        assert_eq!(
            template_path("раздел").unwrap(),
            PathBuf::from("templates/раздел.typ")
        );
    }
}
