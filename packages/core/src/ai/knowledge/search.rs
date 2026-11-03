//! Локальный поиск по формулировкам (принцип IX: поиск не ходит в сеть, FR-007). Слова
//! сравниваются по основе — первым шести буквам без регистра: «замыкание» и «замыкания»
//! находят друг друга без словаря.

use std::collections::HashSet;

use super::store::{Projection, PropositionEntry};

/// Слова короче этого в поиск не идут.
const MIN_WORD_CHARS: usize = 3;
/// Длина основы.
const STEM_CHARS: usize = 6;
/// Слов запроса, которые учитываются (текст пачки длинный).
const MAX_QUERY_WORDS: usize = 4_000;

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.chars().count() >= MIN_WORD_CHARS)
        .map(|word| word.chars().flat_map(char::to_lowercase).collect())
}

fn stem(word: &str) -> String {
    word.chars().take(STEM_CHARS).collect()
}

fn stems_of(text: &str) -> HashSet<String> {
    words(text)
        .take(MAX_QUERY_WORDS)
        .map(|word| stem(&word))
        .collect()
}

/// Утверждения, похожие на текст по общим словам: больше общих основ — выше. Не больше
/// `limit`, порядок устойчив (по числу совпадений, потом по id). `allow` отсекает
/// утверждения, которые показывать нельзя.
pub fn related(
    projection: &Projection,
    text: &str,
    limit: usize,
    allow: impl Fn(&str, &PropositionEntry) -> bool,
) -> Vec<String> {
    let wanted = stems_of(text);
    if wanted.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(usize, &str)> = projection
        .propositions
        .iter()
        .filter(|(id, entry)| allow(id, entry))
        .filter_map(|(id, entry)| {
            let mut own: HashSet<String> = HashSet::new();
            for phrasing in &entry.phrasings {
                own.extend(stems_of(&phrasing.text));
            }
            let common = own.intersection(&wanted).count();
            (common > 0).then_some((common, id.as_str()))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, id)| id.to_owned())
        .collect()
}

/// Подходит ли утверждение под запрос: каждое слово запроса входит в какую-нибудь
/// формулировку. Пустой запрос подходит всем.
pub fn matches_query(entry: &PropositionEntry, query_words: &[String]) -> bool {
    if query_words.is_empty() {
        return true;
    }
    let lowered: Vec<String> = entry
        .phrasings
        .iter()
        .map(|phrasing| phrasing.text.chars().flat_map(char::to_lowercase).collect())
        .collect();
    query_words
        .iter()
        .all(|word| lowered.iter().any(|text| text.contains(word.as_str())))
}

/// Слова запроса пользователя в нижнем регистре; короткие тоже (поиск по подстроке).
pub fn query_words(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .take(16)
        .map(|word| word.chars().flat_map(char::to_lowercase).collect())
        .collect()
}

/// Подходящие под запрос id; порядок — по id.
pub fn find(projection: &Projection, query: &str) -> Vec<String> {
    let words = query_words(query);
    let mut ids: Vec<String> = projection
        .propositions
        .iter()
        .filter(|(_, entry)| matches_query(entry, &words))
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}
