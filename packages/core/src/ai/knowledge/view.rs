//! Вид графа знаний (фича 054, research.md §5): форма `VaultGraph` заметок 025, чтобы
//! тот же холст и воркер раскладки рисовали её без изменений, таблица цепочек и
//! утверждение с источниками. Поиск локальный, по формулировкам (FR-007).
//!
//! Узлы: утверждение — `GraphNodeKind::Note`, развилка цепочки из нескольких посылок —
//! `GraphNodeKind::Tag` (квадрат на холсте). Цепочка с одной посылкой — прямое ребро.
//! Группы — по источнику: `GraphNode::group` — индекс в [`KnowledgeGraph::groups`].

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::search::{find, query_words};
use super::store::{clip, Projection};
use crate::graph::view::{GraphEdge, GraphNode, GraphNodeKind, GroupColor, VaultGraph};

/// Подпись узла на холсте, символов.
pub const LABEL_MAX_CHARS: usize = 80;

/// Группа узлов — источник.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct KnowledgeGroup {
    /// Путь источника (урока).
    pub source: String,
    /// Подпись: имя файла без расширения.
    pub label: String,
    pub color: GroupColor,
    /// Утверждений в группе среди видимых.
    pub count: u32,
}

/// Граф для карты: форма `VaultGraph` плюс группы и счётчики.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct KnowledgeGraph {
    /// Узлы по возрастанию `id`, рёбра по `(from, to)`; `total_notes` — утверждений в графе
    /// до поиска.
    pub graph: VaultGraph,
    pub groups: Vec<KnowledgeGroup>,
    /// Утверждений, цепочек и источников среди видимого.
    pub propositions: u32,
    pub chains: u32,
    pub sources: u32,
    /// Файлы графа не читаются целиком.
    pub damaged: bool,
}

/// Утверждение или посылка в таблице цепочек.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct StatementRef {
    pub id: String,
    pub text: String,
}

/// Откуда взято.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SourceRef {
    pub path: String,
    /// Номер события урока: к нему прокручивается лента.
    pub event: Option<u32>,
    /// Урок удалён, а вклад оставлен.
    pub removed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Chain {
    pub id: String,
    /// Оставшиеся посылки: удалённые не показываются.
    pub premises: Vec<StatementRef>,
    pub conclusion: StatementRef,
    pub sources: Vec<SourceRef>,
    /// Часть посылок удалена.
    pub incomplete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Proposition {
    pub id: String,
    /// Первая формулировка.
    pub text: String,
    /// Все формулировки, различные по написанию.
    pub phrasings: Vec<String>,
    pub sources: Vec<SourceRef>,
    /// Цепочки, где утверждение посылка или вывод.
    pub chains: Vec<Chain>,
}

const COLORS: [GroupColor; 7] = [
    GroupColor::Orange,
    GroupColor::Blue,
    GroupColor::Green,
    GroupColor::Amber,
    GroupColor::Red,
    GroupColor::Hivis,
    GroupColor::Steel,
];

fn primary_text(projection: &Projection, id: &str) -> Option<String> {
    projection
        .propositions
        .get(id)?
        .phrasings
        .first()
        .map(|phrasing| phrasing.text.clone())
}

fn file_label(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .map_or(name, |(stem, _)| stem)
        .to_owned()
}

/// Цепочка в виде таблицы, или `None`, если вывод удалён.
fn chain_of(projection: &Projection, id: &str) -> Option<Chain> {
    let entry = projection.chains.get(id)?;
    let conclusion = StatementRef {
        id: entry.conclusion.clone(),
        text: primary_text(projection, &entry.conclusion)?,
    };
    let premises: Vec<StatementRef> = entry
        .premises
        .iter()
        .filter_map(|premise| {
            Some(StatementRef {
                id: premise.clone(),
                text: primary_text(projection, premise)?,
            })
        })
        .collect();
    if premises.is_empty() {
        return None;
    }
    let mut sources: Vec<SourceRef> = Vec::new();
    for origin in &entry.origins {
        if !sources
            .iter()
            .any(|known| known.path == origin.source && known.event == origin.event)
        {
            sources.push(SourceRef {
                path: origin.source.clone(),
                event: origin.event,
                removed: projection.removed_sources.contains(&origin.source),
            });
        }
    }
    Some(Chain {
        id: id.to_owned(),
        incomplete: premises.len() < entry.premises.len(),
        premises,
        conclusion,
        sources,
    })
}

/// Цепочки, которые видны при поиске: затрагивающие найденные утверждения. Порядок — по id.
fn visible_chains<'a>(
    projection: &'a Projection,
    matched: &std::collections::HashSet<&str>,
    filtered: bool,
) -> Vec<&'a str> {
    let mut ids: Vec<&str> = projection
        .chains
        .iter()
        .filter(|(_, entry)| {
            !filtered
                || matched.contains(entry.conclusion.as_str())
                || entry
                    .premises
                    .iter()
                    .any(|premise| matched.contains(premise.as_str()))
        })
        .map(|(id, _)| id.as_str())
        .collect();
    ids.sort_unstable();
    ids
}

/// Граф для карты. Поиск оставляет подходящие утверждения и их соседей по цепочкам.
pub fn graph(projection: &Projection, query: &str) -> KnowledgeGraph {
    let filtered = !query_words(query).is_empty();
    let found = if filtered {
        find(projection, query)
    } else {
        let mut ids: Vec<String> = projection.propositions.keys().cloned().collect();
        ids.sort();
        ids
    };
    let matched: std::collections::HashSet<&str> = found.iter().map(String::as_str).collect();

    // Цепочки, у которых остаётся хотя бы одна посылка и вывод; их концы тоже видны.
    let mut visible: BTreeSet<&str> = matched.clone().into_iter().collect();
    let mut chains: Vec<Chain> = Vec::new();
    for id in visible_chains(projection, &matched, filtered) {
        if let Some(chain) = chain_of(projection, id) {
            let entry = &projection.chains[id];
            visible.insert(entry.conclusion.as_str());
            for premise in &entry.premises {
                if projection.propositions.contains_key(premise) {
                    visible.insert(premise.as_str());
                }
            }
            chains.push(chain);
        }
    }
    visible.remove("");

    // Группы — источники видимых утверждений, в порядке первого появления по пути.
    let mut group_of: HashMap<String, u32> = HashMap::new();
    let mut groups: Vec<KnowledgeGroup> = Vec::new();
    let mut paths: Vec<String> = visible
        .iter()
        .filter_map(|id| {
            projection
                .propositions
                .get(*id)?
                .phrasings
                .first()
                .map(|p| p.source.clone())
        })
        .collect();
    paths.sort();
    paths.dedup();
    for path in paths {
        group_of.insert(path.clone(), groups.len() as u32);
        groups.push(KnowledgeGroup {
            label: file_label(&path),
            source: path,
            color: COLORS[groups.len() % COLORS.len()],
            count: 0,
        });
    }

    // Узлы: утверждения и развилки, по возрастанию id.
    struct Pending {
        id: String,
        kind: GraphNodeKind,
        label: String,
        path: Option<String>,
        group: Option<u32>,
    }
    let mut nodes: Vec<Pending> = Vec::new();
    for id in &visible {
        let Some(entry) = projection.propositions.get(*id) else {
            continue;
        };
        let Some(first) = entry.phrasings.first() else {
            continue;
        };
        let group = group_of.get(&first.source).copied();
        if let Some(group) = group {
            groups[group as usize].count += 1;
        }
        nodes.push(Pending {
            id: (*id).to_owned(),
            kind: GraphNodeKind::Note,
            label: clip(&first.text, LABEL_MAX_CHARS),
            path: Some(first.source.clone()),
            group,
        });
    }
    for chain in &chains {
        if chain.premises.len() > 1 {
            nodes.push(Pending {
                id: chain.id.clone(),
                kind: GraphNodeKind::Tag,
                label: String::new(),
                path: None,
                group: None,
            });
        }
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let index: HashMap<&str, u32> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i as u32))
        .collect();

    // Рёбра: направленные пары, затем двусторонние сливаются в одно.
    let mut directed: BTreeSet<(u32, u32)> = BTreeSet::new();
    for chain in &chains {
        let Some(&to) = index.get(chain.conclusion.id.as_str()) else {
            continue;
        };
        if chain.premises.len() > 1 {
            let Some(&fork) = index.get(chain.id.as_str()) else {
                continue;
            };
            for premise in &chain.premises {
                if let Some(&from) = index.get(premise.id.as_str()) {
                    directed.insert((from, fork));
                }
            }
            directed.insert((fork, to));
        } else if let Some(&from) = index.get(chain.premises[0].id.as_str()) {
            directed.insert((from, to));
        }
    }
    let mut edges: Vec<GraphEdge> = Vec::new();
    for &(from, to) in &directed {
        if from == to {
            continue;
        }
        let reverse = directed.contains(&(to, from));
        if reverse && from > to {
            continue;
        }
        edges.push(GraphEdge {
            from,
            to,
            both_ways: reverse,
        });
    }
    edges.sort_by_key(|edge| (edge.from, edge.to));
    let mut degree = vec![0u32; nodes.len()];
    for edge in &edges {
        degree[edge.from as usize] += 1;
        degree[edge.to as usize] += 1;
    }
    let propositions = nodes
        .iter()
        .filter(|node| node.kind == GraphNodeKind::Note)
        .count() as u32;
    let sources = groups.iter().filter(|group| group.count > 0).count() as u32;
    KnowledgeGraph {
        graph: VaultGraph {
            nodes: nodes
                .into_iter()
                .zip(degree)
                .map(|(node, degree)| GraphNode {
                    id: node.id,
                    kind: node.kind,
                    label: node.label,
                    path: node.path,
                    degree,
                    group: node.group,
                })
                .collect(),
            edges,
            center: None,
            total_notes: projection.propositions.len() as u32,
        },
        groups,
        propositions,
        chains: chains.len() as u32,
        sources,
        damaged: projection.damaged,
    }
}

/// Таблица цепочек. Поиск оставляет цепочки, где найденное утверждение — посылка или
/// вывод.
pub fn chains(projection: &Projection, query: &str) -> Vec<Chain> {
    let filtered = !query_words(query).is_empty();
    let found = if filtered {
        find(projection, query)
    } else {
        Vec::new()
    };
    let matched: std::collections::HashSet<&str> = found.iter().map(String::as_str).collect();
    visible_chains(projection, &matched, filtered)
        .into_iter()
        .filter_map(|id| chain_of(projection, id))
        .collect()
}

/// Утверждение с формулировками, источниками и цепочками.
pub fn proposition(projection: &Projection, id: &str) -> Option<Proposition> {
    let entry = projection.propositions.get(id)?;
    let mut phrasings: Vec<String> = Vec::new();
    let mut sources: Vec<SourceRef> = Vec::new();
    for phrasing in &entry.phrasings {
        if !phrasings.contains(&phrasing.text) {
            phrasings.push(phrasing.text.clone());
        }
        if !sources
            .iter()
            .any(|known| known.path == phrasing.source && known.event == phrasing.event)
        {
            sources.push(SourceRef {
                path: phrasing.source.clone(),
                event: phrasing.event,
                removed: projection.removed_sources.contains(&phrasing.source),
            });
        }
    }
    let mut chain_ids: Vec<&str> = projection
        .chains
        .iter()
        .filter(|(_, chain)| {
            chain.conclusion == id || chain.premises.iter().any(|premise| premise == id)
        })
        .map(|(chain_id, _)| chain_id.as_str())
        .collect();
    chain_ids.sort_unstable();
    Some(Proposition {
        id: id.to_owned(),
        text: phrasings.first().cloned().unwrap_or_default(),
        phrasings,
        sources,
        chains: chain_ids
            .into_iter()
            .filter_map(|chain_id| chain_of(projection, chain_id))
            .collect(),
    })
}
