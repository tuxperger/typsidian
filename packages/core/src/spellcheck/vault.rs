//! Словарь vault и настройки проверки (фича 030, research.md §4, §5).
//!
//! Словарь — `.typsidian/dictionary.txt`, слово в строке. Это обычный файл
//! vault: его видно без приложения и его везёт синхронизация 011, поэтому
//! слово, добавленное на одном устройстве, не подчёркнуто и на другом (FR-005).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{folded, SpellChecker, SpellConfig, SpellLanguage};
use crate::vault::backend::{VaultBackend, WriteError};
use crate::vault::service_dir::{service_dir, DICTIONARY_FILE_NAME};
use crate::vault::AccessState;

/// Самое длинное слово словаря vault, в символах.
const MAX_WORD_CHARS: usize = 64;

/// Отказ правки словаря vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SpellError {
    /// Не слово: пусто, пробел, цифры, знаки или длиннее 64 символов.
    InvalidWord,
    /// Vault только для чтения.
    ReadOnly,
    Io,
}

pub fn dictionary_path() -> PathBuf {
    service_dir().join(DICTIONARY_FILE_NAME)
}

/// Годится ли строка в словарь vault (data-model.md).
pub fn valid_word(word: &str) -> bool {
    let count = word.chars().count();
    (1..=MAX_WORD_CHARS).contains(&count)
        && word.chars().any(char::is_alphabetic)
        && word
            .chars()
            .all(|c| c.is_alphabetic() || matches!(c, '-' | '\'' | '’'))
}

/// Слова файла: без пустых строк и строк-комментариев `#`.
async fn read_words<B: VaultBackend>(backend: &B) -> Vec<String> {
    match backend.read(&dictionary_path()).await {
        Ok(text) => text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_owned)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Без повторов (без учёта регистра, первое написание остаётся), по алфавиту.
fn normalized(words: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<String> = words
        .into_iter()
        .filter(|word| seen.insert(folded(word)))
        .collect();
    out.sort_by_key(|word| folded(word));
    out
}

async fn write_words<B: VaultBackend>(backend: &mut B, words: &[String]) -> Result<(), SpellError> {
    let mut text = words.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    let failed = |error: WriteError| match error {
        WriteError::AccessDenied => SpellError::ReadOnly,
        _ => SpellError::Io,
    };
    backend
        .ensure_folder(&service_dir())
        .await
        .map_err(failed)?;
    backend
        .write_bytes(&dictionary_path(), text.as_bytes())
        .await
        .map_err(failed)
}

/// Читает настройки vault и словарь vault и настраивает проверяющего (US3).
/// Зовётся при открытии vault, после смены настроек и после внешней правки
/// словаря.
pub async fn spell_configure<B: VaultBackend>(
    backend: &B,
    checker: &mut SpellChecker,
) -> SpellConfig {
    let settings = crate::vault::vault_settings::read_vault_settings(backend)
        .await
        .unwrap_or_default();
    let languages: Vec<SpellLanguage> = settings
        .spellcheck_languages
        .unwrap_or_default()
        .iter()
        .filter_map(|code| SpellLanguage::from_code(code))
        .collect();
    let languages = if languages.is_empty() {
        SpellLanguage::ALL.to_vec()
    } else {
        languages
    };
    checker.configure(
        settings.spellcheck_enabled.unwrap_or(true),
        languages,
        normalized(read_words(backend).await),
    );
    checker.config()
}

async fn change<B: VaultBackend>(
    backend: &mut B,
    checker: &mut SpellChecker,
    word: &str,
    add: bool,
) -> Result<SpellConfig, SpellError> {
    let word = word.trim();
    if !valid_word(word) {
        return Err(SpellError::InvalidWord);
    }
    if backend.access_state() != AccessState::ReadWrite {
        return Err(SpellError::ReadOnly);
    }
    let mut words = read_words(backend).await;
    if add {
        words.push(word.to_owned());
    } else {
        let gone = folded(word);
        words.retain(|other| folded(other) != gone);
    }
    let words = normalized(words);
    write_words(backend, &words).await?;
    let config = checker.config();
    checker.configure(config.enabled, config.languages, words);
    Ok(checker.config())
}

/// Добавляет слово в словарь vault (US2).
pub async fn spell_add_word<B: VaultBackend>(
    backend: &mut B,
    checker: &mut SpellChecker,
    word: &str,
) -> Result<SpellConfig, SpellError> {
    change(backend, checker, word, true).await
}

/// Убирает слово из словаря vault — оно снова проверяется (US2).
pub async fn spell_remove_word<B: VaultBackend>(
    backend: &mut B,
    checker: &mut SpellChecker,
    word: &str,
) -> Result<SpellConfig, SpellError> {
    change(backend, checker, word, false).await
}
