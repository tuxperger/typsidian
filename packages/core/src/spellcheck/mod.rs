//! Проверка орфографии (фича 030).
//!
//! Слова сверяются со словарями Hunspell (`spellbook`, research.md §1) и со
//! словарём vault. Что считать текстом, решает тот же разбор Typst, которым
//! ядро собирает и подсвечивает заметку (024): проверяются только листья текста
//! разметки, а формулы, код, сырой текст, метки, ссылки, адреса и вики-ссылки
//! `[[…]]` — нет (research.md §2). Язык слова — по азбуке (research.md §3).
//!
//! Словари общие на процесс и неизменяемые. Нативной сборке они вшиты; wasm
//! приложения получает их файлами рядом с модулем и отдаёт в
//! [`load_dictionary`] — как шрифты предпросмотра (005 T029). Тестам обоих
//! таргетов словари вшиты фичей `embedded-dictionaries`.
//!
//! Смещения ответа — в кодовых единицах UTF-16, как считает редактор.

mod vault;
#[cfg(target_arch = "wasm32")]
pub mod web;

use std::collections::HashSet;
use std::ops::Range;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use spellbook::Dictionary;
use typst::syntax::{LinkedNode, Source, SyntaxKind};

use crate::highlight::HighlightWindow;
use crate::text_offsets::byte_at;

pub use vault::{
    dictionary_path, spell_add_word, spell_configure, spell_remove_word, valid_word, SpellError,
};

/// Сколько исправлений предлагать (FR-003).
pub const MAX_SUGGESTIONS: usize = 5;

/// Язык проверки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "lowercase")]
pub enum SpellLanguage {
    Ru,
    En,
}

impl SpellLanguage {
    pub const ALL: [SpellLanguage; 2] = [SpellLanguage::Ru, SpellLanguage::En];

    /// Код в настройках vault (`spellcheck.languages`).
    pub fn code(self) -> &'static str {
        match self {
            SpellLanguage::Ru => "ru",
            SpellLanguage::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|language| language.code() == code)
    }
}

/// Действующие настройки проверки — ответ клиенту (data-model.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SpellConfig {
    pub enabled: bool,
    pub languages: Vec<SpellLanguage>,
    /// Словарь vault в порядке файла.
    pub words: Vec<String>,
}

// ── Словари ─────────────────────────────────────────────────────────────────

static RU: OnceLock<Dictionary> = OnceLock::new();
static EN: OnceLock<Dictionary> = OnceLock::new();

fn slot(language: SpellLanguage) -> &'static OnceLock<Dictionary> {
    match language {
        SpellLanguage::Ru => &RU,
        SpellLanguage::En => &EN,
    }
}

/// Словарь языка; `None` — на wasm он ещё не загружен (тогда слова этого
/// языка не проверяются, а не считаются ошибками).
#[cfg(any(not(target_arch = "wasm32"), feature = "embedded-dictionaries"))]
fn dictionary(language: SpellLanguage) -> Option<&'static Dictionary> {
    Some(slot(language).get_or_init(|| {
        let (aff, dic) = match language {
            SpellLanguage::Ru => (
                include_str!("../../dictionaries/ru_RU.aff"),
                include_str!("../../dictionaries/ru_RU.dic"),
            ),
            SpellLanguage::En => (
                include_str!("../../dictionaries/en_US.aff"),
                include_str!("../../dictionaries/en_US.dic"),
            ),
        };
        Dictionary::new(aff, dic).expect("встроенный словарь разбирается")
    }))
}

#[cfg(all(target_arch = "wasm32", not(feature = "embedded-dictionaries")))]
fn dictionary(language: SpellLanguage) -> Option<&'static Dictionary> {
    slot(language).get()
}

/// Загружает словарь языка из текстов `.aff` и `.dic` (wasm приложения).
/// Повторная загрузка — не ошибка, словарь остаётся прежним. Где словари
/// вшиты, вызов ничего не делает.
pub fn load_dictionary(language: SpellLanguage, aff: &str, dic: &str) -> Result<(), String> {
    if slot(language).get().is_some() {
        return Ok(());
    }
    let dictionary = Dictionary::new(aff, dic).map_err(|error| error.to_string())?;
    let _ = slot(language).set(dictionary);
    Ok(())
}

/// Загружен ли словарь языка (на native — всегда).
pub fn dictionary_ready(language: SpellLanguage) -> bool {
    dictionary(language).is_some()
}

// ── Проверяющий ─────────────────────────────────────────────────────────────

/// Настройки проверки и словарь vault. Один на открытый vault: на web — объект
/// клиента, на Tauri — состояние оболочки (research.md §5).
#[derive(Debug, Clone)]
pub struct SpellChecker {
    enabled: bool,
    languages: Vec<SpellLanguage>,
    words: Vec<String>,
    /// Слова vault в нижнем регистре и без «ё» — для сравнения.
    known: HashSet<String>,
}

impl Default for SpellChecker {
    fn default() -> Self {
        Self {
            enabled: true,
            languages: SpellLanguage::ALL.to_vec(),
            words: Vec::new(),
            known: HashSet::new(),
        }
    }
}

/// Итог проверки слова.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Не проверяется: аббревиатура, цифры, две азбуки, язык выключен…
    Skip,
    Correct,
    Wrong,
}

/// Форма слова для сравнения со словарём vault.
pub(crate) fn folded(word: &str) -> String {
    word.to_lowercase().replace('ё', "е").replace('’', "'")
}

impl SpellChecker {
    pub fn configure(&mut self, enabled: bool, languages: Vec<SpellLanguage>, words: Vec<String>) {
        self.enabled = enabled;
        self.languages = languages;
        self.known = words.iter().map(|word| folded(word)).collect();
        self.words = words;
    }

    pub fn config(&self) -> SpellConfig {
        SpellConfig {
            enabled: self.enabled,
            languages: self.languages.clone(),
            words: self.words.clone(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Проверка одного слова по правилам research.md §3.
    pub fn verdict(&self, word: &str) -> Verdict {
        let letters = word.chars().filter(|c| c.is_alphabetic()).count();
        if letters < 2 || word.chars().any(|c| c.is_numeric()) {
            return Verdict::Skip;
        }
        if !word.chars().any(char::is_lowercase) {
            // Целиком в верхнем регистре — аббревиатура.
            return Verdict::Skip;
        }
        let Some(language) = language_of(word) else {
            return Verdict::Skip;
        };
        if !self.languages.contains(&language) {
            return Verdict::Skip;
        }
        let Some(dictionary) = dictionary(language) else {
            return Verdict::Skip;
        };
        if self.correct(dictionary, word) {
            return Verdict::Correct;
        }
        // Через дефис: верно, если верна каждая часть.
        if word.contains('-') {
            let parts = word.split('-').filter(|part| !part.is_empty());
            if parts
                .map(|part| self.verdict(part))
                .all(|verdict| verdict != Verdict::Wrong)
            {
                return Verdict::Correct;
            }
        }
        Verdict::Wrong
    }

    fn correct(&self, dictionary: &Dictionary, word: &str) -> bool {
        if self.known.contains(&folded(word)) {
            return true;
        }
        let word = word.replace('’', "'");
        if dictionary.check(&word) {
            return true;
        }
        if word.contains(['ё', 'Ё']) {
            let plain = word.replace('ё', "е").replace('Ё', "Е");
            return dictionary.check(&plain);
        }
        false
    }

    /// Исправления для слова, до [`MAX_SUGGESTIONS`] (FR-003).
    pub fn suggest(&self, word: &str) -> Vec<String> {
        let Some(dictionary) = language_of(word).and_then(dictionary) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        dictionary.suggest(&word.replace('’', "'"), &mut found);
        found.retain(|candidate| candidate != word);
        found.dedup();
        found.truncate(MAX_SUGGESTIONS);
        found
    }

    /// Ошибки в окнах уже разобранного текста: пары `[from, to)` в UTF-16.
    pub fn misspelled_in_source(&self, source: &Source, windows: &[HighlightWindow]) -> Vec<u32> {
        if !self.enabled {
            return Vec::new();
        }
        let text = source.text();
        let lines = source.lines();
        let to_byte = |utf16: usize| {
            lines
                .utf16_to_byte(utf16)
                .unwrap_or_else(|| byte_at(text, utf16))
        };
        let ranges: Vec<Range<usize>> = if windows.is_empty() {
            vec![0..text.len()]
        } else {
            windows
                .iter()
                .map(|window| to_byte(window.from)..to_byte(window.to.max(window.from)))
                .collect()
        };

        let mut segments = Vec::new();
        collect_text(&LinkedNode::new(source.root()), &ranges, &mut segments);
        let segments = merge(segments);

        // Вики-ссылки для Typst — обычный текст: их диапазоны вычитаются.
        let mut links: Vec<Range<usize>> = Vec::new();
        for range in &ranges {
            let start = line_start(text, range.start);
            let end = line_end(text, range.end);
            links.extend(
                crate::graph::link_sites(&text[start..end])
                    .into_iter()
                    .map(|site| start + site.byte_start..start + site.byte_end),
            );
            links.extend(urls_in(&text[start..end], start));
        }

        let mut out = Vec::new();
        for segment in segments {
            for word in words_in(text, segment) {
                if !ranges.iter().any(|range| overlaps(range, &word))
                    || links.iter().any(|link| overlaps(link, &word))
                {
                    continue;
                }
                if self.verdict(&text[word.clone()]) == Verdict::Wrong {
                    let from = lines.byte_to_utf16(word.start).unwrap_or(0);
                    let to = lines.byte_to_utf16(word.end).unwrap_or(from);
                    out.extend([from as u32, to as u32]);
                }
            }
        }
        out
    }
}

/// Ошибки в окнах текста без сессии подсветки (тесты, бенч).
pub fn misspelled_in(text: &str, windows: &[HighlightWindow], checker: &SpellChecker) -> Vec<u32> {
    checker.misspelled_in_source(&Source::detached(text), windows)
}

/// Исправления слова для открытого vault (FR-003).
pub fn spell_suggest(checker: &SpellChecker, word: &str) -> Vec<String> {
    checker.suggest(word)
}

// ── Слова и азбуки ──────────────────────────────────────────────────────────

fn is_cyrillic(c: char) -> bool {
    matches!(c, '\u{0400}'..='\u{04FF}' | '\u{0500}'..='\u{052F}')
}

fn is_latin(c: char) -> bool {
    c.is_ascii_alphabetic() || matches!(c, '\u{00C0}'..='\u{024F}')
}

/// Язык по азбуке; две азбуки или другая — `None`.
fn language_of(word: &str) -> Option<SpellLanguage> {
    let letters = word.chars().filter(|c| c.is_alphabetic());
    let mut language = None;
    for c in letters {
        let this = if is_cyrillic(c) {
            SpellLanguage::Ru
        } else if is_latin(c) {
            SpellLanguage::En
        } else {
            return None;
        };
        match language {
            None => language = Some(this),
            Some(seen) if seen != this => return None,
            Some(_) => {}
        }
    }
    language
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

fn is_joiner(c: char) -> bool {
    matches!(c, '-' | '\'' | '’')
}

/// Слова отрезка: буквы и цифры с внутренними `-`, `'`, `’`. Байтовые диапазоны.
fn words_in(text: &str, segment: Range<usize>) -> Vec<Range<usize>> {
    let slice = &text[segment.clone()];
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut last_word_end = 0;
    let mut chars = slice.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if is_word_char(c) {
            if start.is_none() {
                start = Some(at);
            }
            last_word_end = at + c.len_utf8();
        } else if is_joiner(c)
            && start.is_some()
            && chars.peek().is_some_and(|&(_, next)| is_word_char(next))
        {
            // Внутренний дефис или апостроф — слово продолжается.
        } else if let Some(from) = start.take() {
            out.push(segment.start + from..segment.start + last_word_end);
        }
    }
    if let Some(from) = start {
        out.push(segment.start + from..segment.start + last_word_end);
    }
    out
}

// ── Обход дерева ────────────────────────────────────────────────────────────

fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

/// Листья текста разметки, пересекающие окна (research.md §2). Апострофы
/// (`SmartQuote`) тоже собираются — иначе «don't» распалось бы на «don» и «t».
fn collect_text(node: &LinkedNode, windows: &[Range<usize>], out: &mut Vec<Range<usize>>) {
    let range = node.range();
    if !windows.iter().any(|window| {
        overlaps(window, &range) || (range.is_empty() && window.contains(&range.start))
    }) {
        return;
    }
    match node.kind() {
        // Сырой текст, формулы, комментарии — не текст заметки.
        SyntaxKind::Raw
        | SyntaxKind::Equation
        | SyntaxKind::Math
        | SyntaxKind::LineComment
        | SyntaxKind::BlockComment => {}
        SyntaxKind::Text | SyntaxKind::SmartQuote => out.push(range),
        _ => {
            for child in node.children() {
                collect_text(&child, windows, out);
            }
        }
    }
}

/// Склейка соседних отрезков: слово, разбитое кавычкой, проверяется целиком.
fn merge(mut segments: Vec<Range<usize>>) -> Vec<Range<usize>> {
    segments.sort_by_key(|segment| segment.start);
    let mut out: Vec<Range<usize>> = Vec::with_capacity(segments.len());
    for segment in segments {
        match out.last_mut() {
            Some(last) if last.end == segment.start => last.end = segment.end,
            _ => out.push(segment),
        }
    }
    out
}

/// Адреса `http(s)://…` до пробела. Typst обрывает ссылку на первой не-ASCII
/// букве, и кириллический хвост адреса стал бы текстом (FR-002).
fn urls_in(line: &str, base: usize) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = line[from..].find("http") {
        let start = from + found;
        let rest = &line[start..];
        if rest.starts_with("https://") || rest.starts_with("http://") {
            let end = rest
                .find(char::is_whitespace)
                .map_or(line.len(), |gap| start + gap);
            out.push(base + start..base + end);
            from = end;
        } else {
            from = start + "http".len();
        }
    }
    out
}

fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |newline| newline + 1)
}

fn line_end(text: &str, at: usize) -> usize {
    text[at..]
        .find('\n')
        .map_or(text.len(), |newline| at + newline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_keep_inner_hyphens_and_apostrophes() {
        let text = "по-русски, don't — «слово-» 12abc";
        let words: Vec<&str> = words_in(text, 0..text.len())
            .into_iter()
            .map(|range| &text[range])
            .collect();
        assert_eq!(words, ["по-русски", "don't", "слово", "12abc"]);
    }

    #[test]
    fn language_follows_the_alphabet() {
        assert_eq!(language_of("проверка"), Some(SpellLanguage::Ru));
        assert_eq!(language_of("check"), Some(SpellLanguage::En));
        assert_eq!(language_of("Wi-Fi-сеть"), None);
    }
}
