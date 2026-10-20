//! Метки связанных файлов (research.md §3): файлы `.typ` из зависимостей
//! последней сборки, разобранные `structure` (015).
//!
//! Байты берутся тем же порядком, каким их подкладывает сборка: из кэша
//! связанных файлов, если отметка файла совпала, иначе с диска. Отказ чтения
//! не ошибка — путь уходит в `unreadable`, и по нему отметки «метки нет» не
//! ставятся.

use std::collections::HashMap;
use std::path::Path;

use super::{LinkedFile, LinkedIndex, LinkedLabel};
use crate::preview::{vault_path, PackageCache};
use crate::vault::backend::VaultBackend;

pub async fn linked_index<B: VaultBackend>(
    backend: &B,
    packages: &PackageCache,
    note_path: &str,
    dependencies: &[String],
) -> LinkedIndex {
    let note = note_path.replace('\\', "/");
    let cached: HashMap<String, _> = packages
        .vault_files_of(note_path)
        .into_iter()
        .filter_map(|(id, mark, bytes)| Some((vault_path(id)?, (mark, bytes))))
        .collect();

    let mut paths: Vec<&String> = dependencies
        .iter()
        .filter(|path| {
            Path::new(path.as_str())
                .extension()
                .is_some_and(|extension| extension == "typ")
                && path.replace('\\', "/") != note
        })
        .collect();
    paths.sort();
    paths.dedup();

    let mut files = Vec::new();
    let mut unreadable = Vec::new();
    for path in paths {
        let fresh = match cached.get(path.as_str()) {
            Some((mark, bytes)) if backend.file_mark(Path::new(path)).await == Some(*mark) => {
                Some(bytes.as_slice().to_vec())
            }
            _ => None,
        };
        let bytes = match fresh {
            Some(bytes) => bytes,
            None => match backend.read_bytes(Path::new(path)).await {
                Ok(bytes) => bytes,
                Err(_) => {
                    unreadable.push(path.clone());
                    continue;
                }
            },
        };
        let Ok(text) = String::from_utf8(bytes) else {
            unreadable.push(path.clone());
            continue;
        };
        files.push(LinkedFile {
            path: path.clone(),
            labels: labels_of(&text),
        });
    }
    LinkedIndex {
        note_path: note_path.to_owned(),
        files,
        unreadable,
    }
}

/// Метки текста с местом в файле: строка, колонка и байты.
pub(crate) fn labels_of(text: &str) -> Vec<LinkedLabel> {
    // Один индекс строк на файл: пересчёт от начала текста на каждую метку
    // был бы квадратичным.
    let lines = typst::syntax::Lines::new(text);
    let byte = |utf16: u32| lines.utf16_to_byte(utf16 as usize).unwrap_or(text.len()) as u32;
    crate::structure::parse(text)
        .labels
        .into_iter()
        .map(|label| LinkedLabel {
            byte_start: byte(label.place.from),
            byte_end: byte(label.place.to),
            line: label.place.line,
            column: label.place.column,
            name: label.name,
            kind: label.kind,
            text: label.text,
            top_level: label.top_level,
        })
        .collect()
}
