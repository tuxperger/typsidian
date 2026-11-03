//! Объединение найденного с графом без дублей (фича 054, research.md §3). Ничего не
//! пишет: возвращает строки, которые допишет вызывающий. Результат модели не доверенный,
//! поэтому у всего есть пределы, а ссылки на несуществующее отклоняются.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::store::{
    chain_id, clip, is_valid_id, normalize_text, proposition_id, ChainLine, Projection,
    PropositionLine, MAX_PREMISES, TEXT_MAX_CHARS,
};

/// Пределы того, что принимается от модели за один вызов.
pub const MAX_FINDING_PROPOSITIONS: usize = 40;
pub const MAX_FINDING_CHAINS: usize = 30;
pub const KEY_MAX_CHARS: usize = 40;
/// Ссылка на посылку или вывод: ключ либо id.
const REF_MAX_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundProposition {
    pub key: String,
    pub text: String,
    pub same_as: Option<String>,
    pub event: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundChain {
    pub premises: Vec<String>,
    pub conclusion: String,
    pub event: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Findings {
    pub propositions: Vec<FoundProposition>,
    pub chains: Vec<FoundChain>,
}

fn text_of(value: &Value, max_chars: usize) -> String {
    clip(value.as_str().map(str::trim).unwrap_or_default(), max_chars)
}

fn event_of(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|n| u32::try_from(n).ok())
}

/// Разбирает аргументы `submit_findings`. Нечитаемый JSON — `None`; негодные элементы
/// пропускаются, лишние сверх предела отбрасываются.
pub fn parse_findings(arguments: &str) -> Option<Findings> {
    let input: Value = if arguments.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(arguments).ok()?
    };
    if !input.is_object() {
        return None;
    }
    let propositions = input["propositions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let text = text_of(&item["text"], TEXT_MAX_CHARS);
            if normalize_text(&text).is_empty() {
                return None;
            }
            let same_as = text_of(&item["same_as"], REF_MAX_CHARS);
            Some(FoundProposition {
                key: text_of(&item["key"], KEY_MAX_CHARS),
                text,
                same_as: (!same_as.is_empty()).then_some(same_as),
                event: event_of(&item["event"]),
            })
        })
        .take(MAX_FINDING_PROPOSITIONS)
        .collect();
    let chains = input["chains"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            let premises = item["premises"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|premise| text_of(premise, REF_MAX_CHARS))
                .filter(|premise| !premise.is_empty())
                // С запасом: сверх предела цепочка всё равно отклоняется.
                .take(MAX_PREMISES + 1)
                .collect();
            // `conclusion` — имя из data-model; `conclude` принимается как синоним.
            let conclusion = match text_of(&item["conclusion"], REF_MAX_CHARS) {
                text if text.is_empty() => text_of(&item["conclude"], REF_MAX_CHARS),
                text => text,
            };
            FoundChain {
                premises,
                conclusion,
                event: event_of(&item["event"]),
            }
        })
        .take(MAX_FINDING_CHAINS)
        .collect();
    Some(Findings {
        propositions,
        chains,
    })
}

/// Что надо дописать в файлы и сколько чего получилось.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergePlan {
    pub propositions: Vec<PropositionLine>,
    pub chains: Vec<ChainLine>,
    pub propositions_added: u32,
    pub propositions_merged: u32,
    pub chains_added: u32,
    pub chains_rejected: u32,
}

/// Объединяет найденное из пачки с графом. `events` — номера событий пачки: номер события
/// вне пачки заменяется первым. `known` — id известных утверждений, отправленных в запросе:
/// ссылаться (`same_as`, цепочки) модель может только на них и на ключи этого вызова, иначе
/// угаданный id присоединил бы формулировку к чужому утверждению или к утверждению из закрытой
/// папки (ревью 054, L7).
pub fn merge(
    projection: &Projection,
    findings: &Findings,
    source: &str,
    events: &[u32],
    now: i64,
    known: &HashSet<String>,
) -> MergePlan {
    let mut plan = MergePlan::default();
    let first_event = events.first().copied();
    let event_in_batch = |event: Option<u32>| match event {
        Some(n) if events.contains(&n) => Some(n),
        _ => first_event,
    };

    // Утверждения, ставшие известными в этом проходе.
    let mut created: HashSet<String> = HashSet::new();
    let mut counted: HashSet<String> = HashSet::new();
    let mut keys: HashMap<String, String> = HashMap::new();
    // Уже решённые строки (id, нормализованный текст) этого источника.
    let mut written: HashSet<(String, String)> = HashSet::new();

    for found in &findings.propositions {
        let text = found.text.trim();
        let normalized = normalize_text(text);
        if normalized.is_empty() {
            continue;
        }
        let exists = |id: &str| projection.propositions.contains_key(id) || created.contains(id);
        // `same_as` — id существующего утверждения либо ключ из этого же вызова; ссылка в
        // пустоту игнорируется, и утверждение считается новым.
        let target = found.same_as.as_deref().and_then(|reference| {
            keys.get(reference).cloned().or_else(|| {
                (is_valid_id(reference, "p_") && known.contains(reference) && exists(reference))
                    .then(|| reference.to_owned())
            })
        });
        let id = target.unwrap_or_else(|| proposition_id(text));
        if !found.key.is_empty() {
            keys.insert(found.key.clone(), id.clone());
        }
        let is_new = !exists(&id);
        if counted.insert(id.clone()) {
            if is_new {
                plan.propositions_added += 1;
            } else {
                plan.propositions_merged += 1;
            }
        }
        created.insert(id.clone());
        let already = projection.propositions.get(&id).is_some_and(|entry| {
            entry.phrasings.iter().any(|phrasing| {
                phrasing.source == source && normalize_text(&phrasing.text) == normalized
            })
        });
        if already || !written.insert((id.clone(), normalized)) {
            continue;
        }
        plan.propositions.push(PropositionLine::Phrasing {
            id,
            text: clip(text, TEXT_MAX_CHARS),
            source: source.to_owned(),
            event: event_in_batch(found.event),
            at: now,
        });
    }

    let mut pending_chains: HashSet<String> = HashSet::new();
    for found in &findings.chains {
        let resolve = |reference: &str| -> Option<String> {
            keys.get(reference).cloned().or_else(|| {
                (is_valid_id(reference, "p_")
                    && known.contains(reference)
                    && projection.propositions.contains_key(reference))
                .then(|| reference.to_owned())
            })
        };
        let premises: Option<Vec<String>> = found
            .premises
            .iter()
            .map(|premise| resolve(premise))
            .collect();
        let conclusion = resolve(&found.conclusion);
        let (Some(mut premises), Some(conclusion)) = (premises, conclusion) else {
            plan.chains_rejected += 1;
            continue;
        };
        let mut seen: HashSet<String> = HashSet::new();
        premises.retain(|premise| seen.insert(premise.clone()));
        if premises.is_empty() || premises.len() > MAX_PREMISES || premises.contains(&conclusion) {
            plan.chains_rejected += 1;
            continue;
        }
        let id = chain_id(&premises, &conclusion);
        let known = projection.chains.get(&id);
        let same_source =
            known.is_some_and(|entry| entry.origins.iter().any(|origin| origin.source == source));
        if same_source || !pending_chains.insert(id.clone()) {
            continue;
        }
        if known.is_none() {
            plan.chains_added += 1;
        }
        plan.chains.push(ChainLine::Chain {
            id,
            premises,
            conclusion,
            source: source.to_owned(),
            event: event_in_batch(found.event),
            at: now,
        });
    }
    plan
}
