//! Упоминания названия заметки без ссылки (research.md §5, §7, §8).
//!
//! Поиск по vault — отменяемый проход порциями без объекта между вызовами:
//! состояние ([`MentionScan`]) маленькое и ходит туда и обратно через границу
//! wasm/Tauri, как значение пакета экспорта. Часов в ядре под wasm нет, поэтому
//! бюджет шага — в заметках и байтах, а не во времени. Проход ничего не пишет,
//! поэтому отмена не может оставить vault в промежуточном состоянии
//! (принцип X).

use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::snippet::{place_of, window};
use super::{
    fetch, group_of, path_string, proximity_key, sort_by_proximity, title_of, Fetched, OpenText,
    SiteRow, SourceGroup,
};
use crate::graph::{link_sites, LinkSite};
use crate::vault::backend::VaultBackend;
use crate::vault::FileKind;

/// Название короче этого числа символов упоминаний не ищет (FR-011).
pub const MIN_MENTION_TITLE_CHARS: usize = 3;
/// Не больше стольких заметок за один шаг прохода.
pub const MENTION_STEP_NOTES: usize = 64;
/// Не больше стольких байт текста за один шаг прохода (одна заметка может его
/// превысить: шаг без единой заметки не продвинулся бы).
pub const MENTION_STEP_BYTES: usize = 1024 * 1024;

/// Можно ли искать упоминания этой заметки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum MentionsAvailability {
    Available {
        title: String,
    },
    /// Название короче [`MIN_MENTION_TITLE_CHARS`] символов (FR-011).
    TitleTooShort {
        title: String,
    },
}

/// Доступность поиска упоминаний. Чистая: ничего не читает.
pub fn availability(target: &str) -> MentionsAvailability {
    let title = title_of(target);
    if title.chars().count() < MIN_MENTION_TITLE_CHARS {
        MentionsAvailability::TitleTooShort { title }
    } else {
        MentionsAvailability::Available { title }
    }
}

/// Регулярное выражение названия без учёта регистра — тот же крейт и приём, что
/// у `search::Matcher`.
fn matcher(title: &str) -> Option<Regex> {
    if title.chars().count() < MIN_MENTION_TITLE_CHARS {
        return None;
    }
    Regex::new(&format!("(?i){}", regex::escape(title))).ok()
}

/// Часть слова: буква, цифра или `_`. `\b` не годится — у названия, которое
/// само кончается не буквой (`C++`), он не срабатывает.
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Стоит ли диапазон `start..end` на границах слов и вне ссылок `[[…]]`.
pub(crate) fn is_free_standing(text: &str, start: usize, end: usize, sites: &[LinkSite]) -> bool {
    let (Some(head), Some(tail)) = (text.get(..start), text.get(end..)) else {
        return false;
    };
    if head.chars().next_back().is_some_and(is_word_char)
        || tail.chars().next().is_some_and(is_word_char)
    {
        return false;
    }
    !sites
        .iter()
        .any(|site| start < site.byte_end && end > site.byte_start)
}

fn rows_with(text: &str, matcher: &Regex) -> Vec<SiteRow> {
    let sites = link_sites(text);
    matcher
        .find_iter(text)
        .filter(|found| is_free_standing(text, found.start(), found.end(), &sites))
        .map(|found| SiteRow {
            place: place_of(text, found.start(), found.end()),
            text: found.as_str().to_owned(),
            snippet: window(text, found.start(), found.end()),
        })
        .collect()
}

/// Упоминания `title` в тексте: без учёта регистра, на границах слов, вне
/// ссылок `[[…]]`. Код и формулы не исключаются. Название короче трёх
/// символов упоминаний не даёт.
pub fn mentions_in_text(text: &str, title: &str) -> Vec<SiteRow> {
    match matcher(title) {
        Some(matcher) => rows_with(text, &matcher),
        None => Vec::new(),
    }
}

/// Фаза прохода.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ScanPhase {
    Running,
    Cancelled,
    Finished,
}

/// Состояние прохода по vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MentionScan {
    pub target: String,
    /// Что ищем.
    pub title: String,
    /// Путь последней обработанной заметки; следующий шаг продолжает после неё.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub after: Option<String>,
    pub scanned: u32,
    /// Фиксируется при `start`; появившиеся позже заметки проход может
    /// обработать, и тогда `scanned` прижимается к `total`.
    pub total: u32,
    /// Нечитаемые и сверх предела.
    pub skipped: u32,
    pub phase: ScanPhase,
}

/// Группы одного шага вместе с новым состоянием.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MentionStep {
    pub state: MentionScan,
    /// Группы только этой порции.
    pub found: Vec<SourceGroup>,
}

/// Пути заметок, которые проход просматривает: только `FileKind::Note`, без
/// самой цели (FR-006), в порядке близости папки и пути.
async fn candidates<B: VaultBackend>(backend: &B, target: &str) -> Vec<(String, u64)> {
    let mut notes: Vec<(String, u64)> = backend
        .list()
        .await
        .into_iter()
        .filter(|note| note.kind == FileKind::Note)
        .map(|note| (path_string(&note.path), note.stamp.size))
        .filter(|(path, _)| path != target)
        .collect();
    notes.sort_by_cached_key(|(path, _)| proximity_key(target, path));
    notes
}

/// Начинает проход. Название короче трёх символов — `Err`, ничего не читается
/// (FR-011).
pub async fn start<B: VaultBackend>(
    backend: &B,
    target: &str,
) -> Result<MentionScan, MentionsAvailability> {
    let title = match availability(target) {
        MentionsAvailability::Available { title } => title,
        refusal => return Err(refusal),
    };
    let total = candidates(backend, target).await.len() as u32;
    Ok(MentionScan {
        target: target.to_owned(),
        title,
        after: None,
        scanned: 0,
        total,
        skipped: 0,
        phase: ScanPhase::Running,
    })
}

/// Ответ на запуск прохода наружу: размеченное объединение, а не `Result` —
/// отказ здесь штатный ответ, а не ошибка вызова.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum MentionsStart {
    Started { state: MentionScan },
    Unavailable { reason: MentionsAvailability },
}

impl From<Result<MentionScan, MentionsAvailability>> for MentionsStart {
    fn from(started: Result<MentionScan, MentionsAvailability>) -> Self {
        match started {
            Ok(state) => Self::Started { state },
            Err(reason) => Self::Unavailable { reason },
        }
    }
}

/// Один шаг: не больше [`MENTION_STEP_NOTES`] заметок и [`MENTION_STEP_BYTES`]
/// прочитанного текста. Ничего не пишет; после `cancel` и `Finished` ничего не
/// читает и возвращает состояние без изменений.
pub async fn step<B: VaultBackend>(
    backend: &B,
    mut state: MentionScan,
    open_texts: &[OpenText],
) -> MentionStep {
    if state.phase != ScanPhase::Running {
        return MentionStep {
            state,
            found: Vec::new(),
        };
    }

    // Заново берём список и пропускаем всё до `after` включительно по ключу
    // порядка: ключ зависит только от путей, поэтому заметки, появившиеся или
    // исчезнувшие между шагами, курсор не ломают.
    let resume = state
        .after
        .as_deref()
        .map(|after| proximity_key(&state.target, after));
    let queue: Vec<(String, u64)> = candidates(backend, &state.target)
        .await
        .into_iter()
        .filter(|(path, _)| {
            resume
                .as_ref()
                .is_none_or(|resume| proximity_key(&state.target, path) > *resume)
        })
        .collect();

    let matcher = matcher(&state.title);
    let mut found = Vec::new();
    let mut bytes = 0usize;
    let mut processed = 0usize;

    for (path, size) in &queue {
        let open = open_texts.iter().find(|open| &open.path == path);
        let size = open.map_or(*size as usize, |open| open.text.len());
        if processed > 0 && (processed >= MENTION_STEP_NOTES || bytes + size > MENTION_STEP_BYTES) {
            break;
        }
        processed += 1;
        bytes += size;
        state.after = Some(path.clone());

        match fetch(backend, path, open_texts).await {
            Fetched::Text(text) => {
                let rows = matcher
                    .as_ref()
                    .map(|matcher| rows_with(&text, matcher))
                    .unwrap_or_default();
                found.extend(group_of(&state.target, path, rows, None));
            }
            Fetched::Problem(_) => state.skipped += 1,
            // Исчезла между списком и чтением — просматривать нечего.
            Fetched::Gone => {}
        }
    }

    state.scanned = (state.scanned + processed as u32).min(state.total);
    if processed == queue.len() {
        state.phase = ScanPhase::Finished;
        state.scanned = state.total;
    }
    MentionStep { state, found }
}

/// Отмена прохода: чистый переход. Завершённый проход отменять нечем.
pub fn cancel(mut state: MentionScan) -> MentionScan {
    if state.phase == ScanPhase::Running {
        state.phase = ScanPhase::Cancelled;
    }
    state
}

/// Упоминания только в названных заметках — точечное обновление списка
/// (research.md §8). Цель, доски и служебные файлы пропускаются; нечитаемые
/// заметки групп не дают.
pub async fn in_notes<B: VaultBackend>(
    backend: &B,
    target: &str,
    paths: &[String],
    open_texts: &[OpenText],
) -> Vec<SourceGroup> {
    let title = match availability(target) {
        MentionsAvailability::Available { title } => title,
        MentionsAvailability::TitleTooShort { .. } => return Vec::new(),
    };
    let Some(matcher) = matcher(&title) else {
        return Vec::new();
    };

    let mut paths: Vec<String> = paths
        .iter()
        .filter(|path| {
            path.as_str() != target && crate::vault::classify(Path::new(path)) == FileKind::Note
        })
        .cloned()
        .collect();
    sort_by_proximity(target, &mut paths);
    paths.dedup();

    let mut groups = Vec::new();
    for path in &paths {
        if let Fetched::Text(text) = fetch(backend, path, open_texts).await {
            groups.extend(group_of(target, path, rows_with(&text, &matcher), None));
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(rows: &[SiteRow]) -> Vec<&str> {
        rows.iter().map(|row| row.text.as_str()).collect()
    }

    #[test]
    fn case_is_ignored_and_the_text_is_kept_as_written() {
        let rows = mentions_in_text("см. проект альфа и ПРОЕКТ АЛЬФА", "Проект Альфа");
        assert_eq!(words(&rows), vec!["проект альфа", "ПРОЕКТ АЛЬФА"]);
    }

    #[test]
    fn word_boundaries_are_checked_by_hand() {
        assert!(mentions_in_text("который", "Кот").is_empty());
        assert!(mentions_in_text("котик", "Кот").is_empty());
        assert!(mentions_in_text("скот_1", "Кот").is_empty());
        assert_eq!(
            words(&mentions_in_text("(кот), «Кот»; кот.", "Кот")).len(),
            3
        );
        // Название кончается не буквой: `\b` тут не сработал бы.
        assert_eq!(
            words(&mentions_in_text("пишу на C++ и Rust", "C++")),
            vec!["C++"]
        );
        assert!(mentions_in_text("C++x", "C++").is_empty());
    }

    #[test]
    fn mentions_inside_links_are_masked() {
        let text = "[[Проект Альфа]] и снова Проект Альфа, [[/a/Проект Альфа]]";
        let rows = mentions_in_text(text, "Проект Альфа");
        assert_eq!(rows.len(), 1);
        let start = text.rfind(", [[").unwrap();
        assert!((rows[0].place.byte_end as usize) <= start);
    }

    #[test]
    fn short_title_yields_nothing() {
        assert!(mentions_in_text("ab ab ab", "ab").is_empty());
        assert!(matches!(
            availability("a/ab.typ"),
            MentionsAvailability::TitleTooShort { .. }
        ));
        assert!(matches!(
            availability("a/abc.typ"),
            MentionsAvailability::Available { .. }
        ));
        // Считаются символы, а не байты: «Кот» — три символа и шесть байт.
        assert!(matches!(
            availability("Кот.typ"),
            MentionsAvailability::Available { .. }
        ));
    }

    #[test]
    fn rows_carry_place_and_highlighted_snippet() {
        let rows = mentions_in_text("первая\nвторая Кот тут", "Кот");
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].place.line, rows[0].place.column), (2, 8));
        assert!(rows[0]
            .snippet
            .iter()
            .any(|s| s.highlighted && s.text == "Кот"));
    }
}
