//! Кандидаты для `@`: метки заметки и связанных файлов, затем ключи источников
//! 017 отдельной группой (research.md §5, FR-001, FR-003).

use std::collections::HashMap;

use super::{
    match_tier, CompletionGroup, CompletionItem, GroupKind, ItemDetail, LinkedIndex, SourceKey,
    GROUP_LIMIT,
};
use crate::structure::LabelKind;

/// Одно объявление метки — до свёртки повторов.
struct Declared<'a> {
    name: &'a str,
    kind: LabelKind,
    text: &'a str,
    /// `None` — в открытой заметке.
    file: Option<&'a str>,
    /// Порядок внутри яруса: у заметки — расстояние в строках от каретки, у
    /// связанного файла — номер файла и строка.
    order: (u8, u32, u32),
}

pub fn candidates(
    text: &str,
    offset: u32,
    query: &str,
    linked: &LinkedIndex,
    sources: &[SourceKey],
) -> Vec<CompletionGroup> {
    candidates_in(
        &crate::structure::parse(text),
        text,
        offset,
        query,
        linked,
        sources,
    )
}

/// То же по уже собранному строению текста.
pub fn candidates_in(
    structure: &crate::structure::NoteStructure,
    text: &str,
    offset: u32,
    query: &str,
    linked: &LinkedIndex,
    sources: &[SourceKey],
) -> Vec<CompletionGroup> {
    let lines = typst::syntax::Lines::new(text);
    let caret_line = lines
        .utf16_to_byte(offset as usize)
        .and_then(|byte| lines.byte_to_line(byte))
        .unwrap_or(0) as u32
        + 1;

    let mut declared: Vec<Declared> = structure
        .labels
        .iter()
        .map(|label| Declared {
            name: &label.name,
            kind: label.kind,
            text: &label.text,
            file: None,
            order: (0, label.place.line.abs_diff(caret_line), 0),
        })
        .collect();
    for (index, file) in linked.files.iter().enumerate() {
        declared.extend(file.labels.iter().map(|label| Declared {
            name: &label.name,
            kind: label.kind,
            text: &label.text,
            file: Some(file.path.as_str()),
            order: (1, index as u32, label.line),
        }));
    }

    // Повторы: одна строка — ближайшее объявление, с признаком повтора.
    let mut times: HashMap<&str, u32> = HashMap::new();
    for item in &declared {
        *times.entry(item.name).or_insert(0) += 1;
    }
    declared.sort_by_key(|item| item.order);
    let mut seen = std::collections::HashSet::new();
    let mut matched: Vec<(u8, (u8, u32, u32), CompletionItem)> = declared
        .into_iter()
        .filter(|item| seen.insert(item.name))
        .filter_map(|item| {
            let (tier, spans) = match_tier(item.name, query)?;
            Some((
                tier,
                item.order,
                CompletionItem {
                    label: item.name.to_owned(),
                    matched: spans,
                    detail: ItemDetail::Label {
                        label_kind: item.kind,
                        text: item.text.to_owned(),
                        file: item.file.map(str::to_owned),
                        duplicate: times[item.name] > 1,
                    },
                    insert: item.name.to_owned(),
                    preview: format!("@{}", item.name),
                },
            ))
        })
        .collect();
    matched.sort_by_key(|(tier, order, _)| (*tier, *order));

    let mut groups = Vec::new();
    push_group(
        &mut groups,
        GroupKind::Labels,
        matched.into_iter().map(|(_, _, item)| item).collect(),
    );

    // Ключи источников: тот же ярус совпадения, внутри — порядок 017.
    let mut keys: Vec<(u8, usize, CompletionItem)> = sources
        .iter()
        .enumerate()
        .filter_map(|(index, source)| {
            let (tier, spans) = match_tier(&source.key, query)?;
            Some((
                tier,
                index,
                CompletionItem {
                    label: source.key.clone(),
                    matched: spans,
                    detail: ItemDetail::Source {
                        summary: source.summary.clone(),
                    },
                    insert: source.key.clone(),
                    preview: format!("@{}", source.key),
                },
            ))
        })
        .collect();
    keys.sort_by_key(|(tier, index, _)| (*tier, *index));
    push_group(
        &mut groups,
        GroupKind::Sources,
        keys.into_iter().map(|(_, _, item)| item).collect(),
    );
    groups
}

/// Группа без строк не отдаётся; строк — не больше [`GROUP_LIMIT`].
pub(crate) fn push_group(
    groups: &mut Vec<CompletionGroup>,
    kind: GroupKind,
    items: Vec<CompletionItem>,
) {
    if items.is_empty() {
        return;
    }
    let total = items.len() as u32;
    groups.push(CompletionGroup {
        kind,
        total,
        items: items.into_iter().take(GROUP_LIMIT).collect(),
    });
}
