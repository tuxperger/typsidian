//! Страница обратных ссылок (research.md §4).
//!
//! Источники берутся из индекса ссылок backend — без чтения файлов, — а текст
//! читается только у заметок запрошенной порции. Поэтому первая порция на
//! vault в тысячи заметок стоит не больше пятидесяти чтений (SC-001).
//! Функция ничего не пишет и не возвращает `Result`: нечитаемый источник —
//! группа с `problem`, несуществующая цель — пустая страница.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::snippet::{place_of, window};
use super::{
    fetch, group_of, path_string, sort_by_proximity, Fetched, OpenText, SiteRow, SourceGroup,
};
use crate::graph::{link_sites, resolve_link_target};
use crate::vault::backend::VaultBackend;
use crate::vault::FileKind;

/// Сколько заметок-источников в одной порции.
pub const BACKLINKS_PAGE_MAX: u32 = 50;

/// Порция обратных ссылок.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BacklinksPage {
    /// Для какой заметки ответ: по нему интерфейс отбрасывает устаревшие
    /// ответы после смены заметки (FR-009).
    pub target: String,
    /// Ссылающихся заметок по индексу — точное число, не число показанных.
    pub total_sources: u32,
    /// Порция, ближайшие по папке первыми.
    pub groups: Vec<SourceGroup>,
    /// Откуда просить следующую порцию; нет — всё показано.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub next_offset: Option<u32>,
}

/// Вхождения ссылок на `target` в тексте `source`, в порядке следования.
pub(crate) fn rows_linking_to(source: &str, text: &str, target: &str) -> Vec<SiteRow> {
    let source_path = Path::new(source);
    let target_path = Path::new(target);
    link_sites(text)
        .into_iter()
        .filter(|site| resolve_link_target(source_path, &site.raw).as_deref() == Some(target_path))
        .map(|site| SiteRow {
            place: place_of(text, site.byte_start, site.byte_end),
            text: text[site.byte_start..site.byte_end].to_owned(),
            snippet: window(text, site.byte_start, site.byte_end),
        })
        .collect()
}

/// Страница обратных ссылок `target`: `limit` источников начиная с `offset`.
///
/// - цель источником не бывает (FR-006);
/// - порядок — близость папки к папке цели, затем путь;
/// - источник, в текущем тексте которого ссылки уже нет (индекс отстал), и
///   источник, которого уже нет на диске, в страницу не попадают;
/// - `open_texts` приоритетнее диска (research.md §10);
/// - `limit` прижимается к `1..=`[`BACKLINKS_PAGE_MAX`].
pub async fn backlinks_page<B: VaultBackend>(
    backend: &B,
    target: &str,
    open_texts: &[OpenText],
    offset: u32,
    limit: u32,
) -> BacklinksPage {
    let limit = limit.clamp(1, BACKLINKS_PAGE_MAX);

    let mut sources: Vec<String> = backend
        .backlinks(Path::new(target))
        .await
        .into_iter()
        .filter(|note| note.kind == FileKind::Note)
        .map(|note| path_string(&note.path))
        .filter(|path| path != target)
        .collect();
    sort_by_proximity(target, &mut sources);
    sources.dedup();

    let total = sources.len() as u32;
    let from = (offset as usize).min(sources.len());
    let to = (from + limit as usize).min(sources.len());

    let mut groups = Vec::new();
    for path in &sources[from..to] {
        let group = match fetch(backend, path, open_texts).await {
            Fetched::Text(text) => {
                group_of(target, path, rows_linking_to(path, &text, target), None)
            }
            Fetched::Problem(problem) => group_of(target, path, Vec::new(), Some(problem)),
            Fetched::Gone => None,
        };
        groups.extend(group);
    }

    BacklinksPage {
        target: target.to_owned(),
        total_sources: total,
        groups,
        next_offset: (to < sources.len()).then_some(to as u32),
    }
}
