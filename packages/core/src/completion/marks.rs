//! Отметки ссылок в никуда и повторных меток (research.md §4, §8, FR-006,
//! FR-007).
//!
//! Текст — единственный мгновенный источник, сборка — единственный
//! достоверный о метках из кода. Свидетельство сборки здесь только
//! подтверждает: снимает или не ставит отметку, но никогда её не создаёт.
//! Поэтому устаревшее свидетельство даёт пропуск отметки, а не ложную.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::{DuplicatePlace, LinkedIndex, MarkKind, MarksRequest, ReferenceMark};
use crate::graph::{link_sites, resolve_link_target};
use crate::preview::evidence::BuildEvidence;
use crate::structure::NoteStructure;
use crate::vault::backend::VaultBackend;

/// Отметки о метках: «метки нет» и «метка объявлена повторно». Чистая часть
/// [`reference_marks`].
pub fn label_marks(request: &MarksRequest, structure: &NoteStructure) -> Vec<ReferenceMark> {
    let mut marks = missing_labels(request, structure);
    marks.extend(duplicate_labels(
        structure,
        &request.linked,
        request.evidence.as_ref(),
    ));
    marks
}

fn missing_labels(request: &MarksRequest, structure: &NoteStructure) -> Vec<ReferenceMark> {
    // Недостающая метка может стоять в непрочитанном файле — по тексту это
    // не отличить (research.md §3).
    if !request.linked.unreadable.is_empty() {
        return Vec::new();
    }
    let known: HashSet<&str> = structure
        .labels
        .iter()
        .map(|label| label.name.as_str())
        .chain(
            request
                .linked
                .files
                .iter()
                .flat_map(|file| file.labels.iter().map(|label| label.name.as_str())),
        )
        .chain(request.sources.iter().map(String::as_str))
        .collect();
    let previously: HashSet<&str> = request
        .previously_missing
        .iter()
        .map(String::as_str)
        .collect();
    structure
        .refs
        .iter()
        .filter(|reference| !known.contains(reference.target.as_str()))
        .filter(|reference| {
            !request
                .evidence
                .as_ref()
                .is_some_and(|evidence| evidence.confirms(&reference.target))
        })
        // Только что набранная ссылка ждёт сборку (удержание, research.md §4).
        .filter(|reference| !request.held || previously.contains(reference.target.as_str()))
        .filter_map(|reference| {
            // Отметка — на `@имя`, без дополнения `[…]`.
            let from = reference.place.from;
            let to = from + 1 + reference.target.encode_utf16().count() as u32;
            let claimed = request
                .claimed
                .iter()
                .any(|&(start, end)| start < to && from < end);
            (!claimed).then(|| ReferenceMark {
                kind: MarkKind::MissingLabel {
                    name: reference.target.clone(),
                },
                from,
                to: to.min(reference.place.to),
                line: reference.place.line,
            })
        })
        .collect()
}

fn duplicate_labels(
    structure: &NoteStructure,
    linked: &LinkedIndex,
    evidence: Option<&BuildEvidence>,
) -> Vec<ReferenceMark> {
    let mut by_name: HashMap<&str, Vec<&crate::structure::LabelSite>> = HashMap::new();
    for label in &structure.labels {
        by_name.entry(label.name.as_str()).or_default().push(label);
    }
    let mut marks = Vec::new();
    for (name, declared) in by_name {
        let in_linked: Vec<DuplicatePlace> = linked
            .files
            .iter()
            .flat_map(|file| {
                file.labels
                    .iter()
                    .filter(move |label| label.name == name)
                    .map(move |label| DuplicatePlace {
                        path: Some(file.path.clone()),
                        line: label.line,
                    })
            })
            .collect();
        let top_level = declared.iter().filter(|label| label.top_level).count();
        let built_twice = evidence.is_some_and(|evidence| evidence.count(name) >= 2);
        // Оба объявления в разметке верхнего уровня — в документе окажутся
        // оба; иначе повтор подтверждает только сборка.
        let marked: Vec<_> = if top_level >= 2 {
            declared.iter().filter(|label| label.top_level).collect()
        } else if built_twice && declared.len() + in_linked.len() >= 2 {
            declared.iter().collect()
        } else {
            continue;
        };
        for label in &marked {
            let others = declared
                .iter()
                .filter(|other| other.place != label.place)
                .map(|other| DuplicatePlace {
                    path: None,
                    line: other.place.line,
                })
                .chain(in_linked.iter().cloned())
                .collect();
            marks.push(ReferenceMark {
                kind: MarkKind::DuplicateLabel {
                    name: name.to_owned(),
                    others,
                },
                from: label.place.from,
                to: label.place.to,
                line: label.place.line,
            });
        }
    }
    marks
}

/// Все отметки заметки: о метках и о ссылках `[[…]]` (contracts/core-completion-api.md).
/// `backend` нужен только для проверки, существует ли заметка-цель.
pub async fn reference_marks<B: VaultBackend>(
    backend: &B,
    request: &MarksRequest,
) -> Vec<ReferenceMark> {
    let structure = crate::structure::parse(&request.text);
    let mut marks = label_marks(request, &structure);

    let sites = link_sites(&request.text);
    if !sites.is_empty() {
        let existing: HashSet<_> = backend
            .list()
            .await
            .into_iter()
            .map(|note| note.path)
            .collect();
        let source = Path::new(&request.note_path);
        let lines = typst::syntax::Lines::new(request.text.as_str());
        let utf16 = |byte: usize| lines.byte_to_utf16(byte).unwrap_or(0) as u32;
        let line_of = |byte: usize| lines.byte_to_line(byte).unwrap_or(0) as u32 + 1;
        for site in sites {
            let kind = match resolve_link_target(source, &site.raw) {
                None => MarkKind::LinkOutsideVault {
                    target: site.raw.clone(),
                },
                // Ссылка на себя граф не считает связью — и отметки нет.
                Some(target) if target == source => continue,
                Some(target) if existing.contains(&target) => continue,
                Some(target) => MarkKind::MissingNote {
                    target: site.raw.clone(),
                    create_path: target.to_string_lossy().replace('\\', "/"),
                },
            };
            marks.push(ReferenceMark {
                kind,
                from: utf16(site.byte_start),
                to: utf16(site.byte_end),
                line: line_of(site.byte_start),
            });
        }
    }

    // По порядку текста и без пересечений: одна запись — одна отметка.
    marks.sort_by_key(|mark| (mark.from, mark.to));
    let mut end = 0;
    marks.retain(|mark| {
        if mark.from < end {
            return false;
        }
        end = mark.to;
        true
    });
    marks
}
