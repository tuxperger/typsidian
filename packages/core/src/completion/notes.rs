//! Кандидаты для `[[`: заметки vault (research.md §6, FR-002).
//!
//! Запись берётся у функции кратчайшей записи ссылки 013 (`graph::link_text`):
//! что она подставит, то граф и разрешит обратно. Доски не предлагаются —
//! ссылка `[[…]]` на доску не разрешается.

use std::path::{Component, Path};

use super::labels::push_group;
use super::{match_tier, CompletionGroup, CompletionItem, GroupKind, ItemDetail};
use crate::graph::link_text;
use crate::vault::index::Note;
use crate::vault::FileKind;

pub fn candidates(
    note_path: &str,
    notes: &[Note],
    query: &str,
    closed: bool,
) -> Vec<CompletionGroup> {
    let source = Path::new(note_path);
    let source_folder = folder_parts(source);

    let mut matched: Vec<((u8, usize, String), CompletionItem)> = notes
        .iter()
        .filter(|note| note.kind == FileKind::Note && note.path != source)
        .filter_map(|note| {
            let name = note.path.file_stem()?.to_str()?;
            let path = note.path.to_str()?.replace('\\', "/");
            // Совпадение в имени; нет — в пути, ярусом ниже.
            let (tier, spans) = match match_tier(name, query) {
                Some(found) => found,
                // Подсвечивать в имени нечего: совпало в папке.
                None => {
                    match_tier(&path, query)?;
                    (3, Vec::new())
                }
            };
            let written = link_text(source, &note.path)?;
            let inner = written.strip_prefix("[[")?.strip_suffix("]]")?.to_owned();
            let folder_parts = folder_parts(&note.path);
            let same_folder = folder_parts == source_folder;
            let folder = folder_parts.join("/");
            Some((
                (tier, steps(&source_folder, &folder_parts), path),
                CompletionItem {
                    label: name.to_owned(),
                    matched: spans,
                    detail: ItemDetail::Note {
                        folder,
                        same_folder,
                    },
                    insert: if closed {
                        inner.clone()
                    } else {
                        format!("{inner}]]")
                    },
                    preview: written,
                },
            ))
        })
        .collect();
    matched.sort_by(|(left, _), (right, _)| left.cmp(right));

    let mut groups = Vec::new();
    push_group(
        &mut groups,
        GroupKind::Notes,
        matched.into_iter().map(|(_, item)| item).collect(),
    );
    groups
}

fn folder_parts(path: &Path) -> Vec<String> {
    path.parent()
        .map(|parent| {
            parent
                .components()
                .filter_map(|part| match part {
                    Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Шагов по дереву между двумя папками.
fn steps(from: &[String], to: &[String]) -> usize {
    let common = from.iter().zip(to).take_while(|(a, b)| a == b).count();
    from.len() + to.len() - 2 * common
}
