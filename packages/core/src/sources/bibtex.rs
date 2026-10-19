//! Чтение BibTeX без потери записей (FR-008, research.md §2).
//!
//! Быстрый путь — штатный разбор `biblatex` целиком. При отказе файл режется
//! на куски по началам записей, и каждый кусок читает тот же крейт: своего
//! разбора здесь нет, наш код только находит границы. Ошибочная запись
//! называется со строкой, остальные остаются.

use std::collections::{HashMap, HashSet};

use biblatex::{Bibliography, RawBibliography};

use super::{source_of, FileReading, LineIndex, SourceReadProblem, SourceReadProblemKind};

pub fn read_bibtex(file: &str, text: &str) -> FileReading {
    let positions = LineIndex::new(text);
    let mut problems = Vec::new();
    // Записи с началом в файле: по нему — строка записи и ошибки.
    let mut entries = Vec::new();
    let mut abbreviations = Vec::new();
    match RawBibliography::parse(text) {
        Ok(raw) => {
            for entry in raw.entries {
                entries.push((entry.span.start, entry));
            }
            abbreviations = raw.abbreviations;
        }
        Err(_) => {
            for (offset, chunk) in chunks(text) {
                match RawBibliography::parse(chunk) {
                    Ok(raw) => {
                        for entry in raw.entries {
                            entries.push((offset + entry.span.start, entry));
                        }
                        abbreviations.extend(raw.abbreviations);
                    }
                    Err(error) => {
                        // Место — начало записи: по нему её ищут в файле.
                        let at = chunk.find('@').unwrap_or(0);
                        let (line, column) = positions.at(offset + at);
                        problems.push(SourceReadProblem {
                            file: file.to_owned(),
                            line,
                            column,
                            key: chunk_key(chunk),
                            kind: SourceReadProblemKind::Syntax {
                                detail: error.kind.to_string(),
                            },
                        });
                    }
                }
            }
        }
    }

    // Повтор ключа: первая запись — источник, следующие — ошибка (правило 4).
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for (start, entry) in entries {
        let key = entry.v.key.v.to_owned();
        if seen.insert(key.clone()) {
            unique.push((start, entry));
        } else {
            let (line, column) = positions.at(start);
            problems.push(SourceReadProblem {
                file: file.to_owned(),
                line,
                column,
                key: Some(key),
                kind: SourceReadProblemKind::DuplicateKey,
            });
        }
    }
    let lines: HashMap<String, (u32, u32)> = unique
        .iter()
        .map(|(start, entry)| (entry.v.key.v.to_owned(), positions.at(*start)))
        .collect();
    let has_entries = !unique.is_empty();

    // Разрешение сокращений и `crossref` — целиком; отказ — по одной записи.
    let raw_entries: Vec<_> = unique.into_iter().map(|(_, entry)| entry).collect();
    let whole = Bibliography::from_raw(RawBibliography {
        preamble: String::new(),
        entries: raw_entries.clone(),
        abbreviations: abbreviations.clone(),
    });
    let resolved: Vec<biblatex::Entry> = match whole {
        Ok(bibliography) => bibliography.into_iter().collect(),
        Err(_) => {
            let mut good = Vec::new();
            for entry in raw_entries {
                let key = entry.v.key.v.to_owned();
                let one = Bibliography::from_raw(RawBibliography {
                    preamble: String::new(),
                    entries: vec![entry],
                    abbreviations: abbreviations.clone(),
                });
                match one {
                    Ok(bibliography) => good.extend(bibliography.into_iter()),
                    Err(error) => {
                        let (line, column) = lines.get(&key).copied().unwrap_or((0, 0));
                        problems.push(SourceReadProblem {
                            file: file.to_owned(),
                            line,
                            column,
                            key: Some(key),
                            kind: SourceReadProblemKind::Syntax {
                                detail: error.kind.to_string(),
                            },
                        });
                    }
                }
            }
            good
        }
    };

    // Сведение к записи Hayagriva — по одной: ошибка типа не трогает соседей.
    let mut sources = Vec::new();
    for entry in &resolved {
        let (line, column) = lines.get(&entry.key).copied().unwrap_or((0, 0));
        match hayagriva::Entry::try_from(entry) {
            Ok(converted) => sources.push(source_of(&converted, file, line)),
            Err(error) => problems.push(SourceReadProblem {
                file: file.to_owned(),
                line,
                column,
                key: Some(entry.key.clone()),
                kind: SourceReadProblemKind::Field {
                    field: String::new(),
                    detail: format!("{:?}", error.kind),
                },
            }),
        }
    }
    problems.sort_by_key(|problem| (problem.line, problem.column));
    FileReading {
        sources,
        problems,
        has_entries,
    }
    .capped(file)
}

/// Куски файла по началам записей: строка, начинающаяся с `@имя{` или
/// `@имя(`. Текст до первой записи (комментарии) — отдельный кусок.
fn chunks(text: &str) -> Vec<(usize, &str)> {
    let mut starts = vec![0];
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if offset > 0 && starts_entry(line) {
            starts.push(offset);
        }
        offset += line.len();
    }
    starts.push(text.len());
    starts
        .windows(2)
        .map(|pair| (pair[0], &text[pair[0]..pair[1]]))
        .filter(|(_, chunk)| !chunk.trim().is_empty())
        .collect()
}

fn starts_entry(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix('@') else {
        return false;
    };
    let name_end = rest
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(rest.len());
    name_end > 0 && rest[name_end..].trim_start().starts_with(['{', '('])
}

/// Ключ записи из её начала: `@article{ключ,` — чтобы назвать ошибку.
fn chunk_key(chunk: &str) -> Option<String> {
    let start = chunk.find('@')?;
    let rest = &chunk[start..];
    let open = rest.find(['{', '('])?;
    let after = &rest[open + 1..];
    let key = after.split([',', '\n', '}', ')']).next()?.trim();
    (!key.is_empty()).then(|| key.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "@book{knuth1984,\n  author = {Knuth, Donald E.},\n  title = {The {\\TeX}book},\n  year = {1984},\n}\n";

    #[test]
    fn a_good_file_reads_entirely() {
        let reading = read_bibtex("refs.bib", GOOD);
        assert!(reading.problems.is_empty(), "{:?}", reading.problems);
        assert_eq!(reading.sources.len(), 1);
        let source = &reading.sources[0];
        assert_eq!(source.key, "knuth1984");
        assert_eq!(source.authors, vec!["Knuth D. E."]);
        assert_eq!(source.year, Some(1984));
        assert_eq!(source.line, 1);
    }

    #[test]
    fn a_broken_entry_does_not_hide_the_others() {
        let text = format!("{GOOD}@article{{broken,\n  title = {{Незакрытая\n\n@misc{{after,\n  title = {{После}},\n  year = 2020,\n}}\n");
        let reading = read_bibtex("refs.bib", &text);
        let keys: Vec<&str> = reading.sources.iter().map(|s| s.key.as_str()).collect();
        assert!(
            keys.contains(&"knuth1984") && keys.contains(&"after"),
            "{keys:?}"
        );
        assert!(reading
            .problems
            .iter()
            .any(|p| p.key.as_deref() == Some("broken") && p.line == 6));
    }

    #[test]
    fn a_repeated_key_is_named() {
        let text = format!("{GOOD}{GOOD}");
        let reading = read_bibtex("refs.bib", &text);
        assert_eq!(reading.sources.len(), 1);
        assert_eq!(reading.problems.len(), 1);
        assert_eq!(
            reading.problems[0].kind,
            SourceReadProblemKind::DuplicateKey
        );
        assert_eq!(reading.problems[0].line, 6);
    }

    #[test]
    fn tex_and_abbreviations_are_resolved() {
        let text = "@string{jn = {Журнал}}\n@article{a,\n  author = {M{\\\"u}ller, Hans},\n  title = {T},\n  journal = jn,\n  year = 2001,\n}\n";
        let reading = read_bibtex("refs.bib", text);
        assert!(reading.problems.is_empty(), "{:?}", reading.problems);
        assert_eq!(reading.sources[0].authors, vec!["Müller H."]);
        assert_eq!(reading.sources[0].container.as_deref(), Some("Журнал"));
    }

    #[test]
    fn arbitrary_bytes_do_not_panic() {
        for text in [
            "@",
            "@{",
            "@a{",
            "}}}{{{",
            "@article{k, title = {",
            "\u{0}\u{1}@x(y",
        ] {
            let _ = read_bibtex("x.bib", text);
        }
    }
}
