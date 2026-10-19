//! Чтение Hayagriva YAML без потери записей (FR-008, research.md §2).
//!
//! Быстрый путь — `hayagriva::io::from_yaml_str`. При отказе YAML разбирается
//! до карты, и каждая пара верхнего уровня читается отдельной библиотекой из
//! одной записи: `Entry` сам не десериализуется, только `Library`. Сломанный
//! синтаксис самого YAML — ошибка уровня файла: границ записей тогда нет.

use super::{source_of, FileReading, SourceReadProblem, SourceReadProblemKind};

pub fn read_hayagriva(file: &str, text: &str) -> FileReading {
    if let Ok(library) = hayagriva::io::from_yaml_str(text) {
        let sources: Vec<_> = library
            .iter()
            .map(|entry| source_of(entry, file, key_line(text, entry.key())))
            .collect();
        return FileReading {
            has_entries: !sources.is_empty(),
            sources,
            problems: Vec::new(),
        };
    }
    let mapping = match serde_yaml::from_str::<serde_yaml::Mapping>(text) {
        Ok(mapping) => mapping,
        Err(error) => {
            let (line, column) = error
                .location()
                .map_or((0, 0), |at| (at.line() as u32, at.column() as u32));
            return FileReading {
                sources: Vec::new(),
                problems: vec![SourceReadProblem {
                    file: file.to_owned(),
                    line,
                    column,
                    key: None,
                    kind: SourceReadProblemKind::FileSyntax {
                        detail: error.to_string(),
                    },
                }],
                has_entries: false,
            };
        }
    };
    let mut reading = FileReading::default();
    for (key, value) in mapping {
        let Some(name) = key.as_str().map(str::to_owned) else {
            continue;
        };
        let line = key_line(text, &name);
        // Запись без вида: самый частый случай, называется отдельно.
        let has_type = value
            .as_mapping()
            .is_some_and(|fields| fields.contains_key("type"));
        let mut one = serde_yaml::Mapping::new();
        one.insert(key, value);
        match serde_yaml::from_value::<hayagriva::Library>(serde_yaml::Value::Mapping(one)) {
            Ok(library) => {
                reading.has_entries = true;
                reading
                    .sources
                    .extend(library.iter().map(|entry| source_of(entry, file, line)));
            }
            Err(error) => reading.problems.push(SourceReadProblem {
                file: file.to_owned(),
                line,
                column: if line > 0 { 1 } else { 0 },
                key: Some(name),
                kind: if has_type {
                    SourceReadProblemKind::Field {
                        field: String::new(),
                        detail: error.to_string(),
                    }
                } else {
                    SourceReadProblemKind::NoEntryType
                },
            }),
        }
    }
    reading.capped(file)
}

/// Строка, где ключ записи стоит в начале строки файла; `0` — не нашлась.
fn key_line(text: &str, key: &str) -> u32 {
    text.lines()
        .position(|line| {
            line.strip_prefix(key)
                .or_else(|| {
                    line.strip_prefix('"')
                        .and_then(|rest| rest.strip_prefix(key))
                        .and_then(|rest| rest.strip_prefix('"'))
                })
                .is_some_and(|rest| rest.trim_start().starts_with(':'))
        })
        .map_or(0, |index| index as u32 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "knuth1984:\n  type: book\n  title: The TeXbook\n  author: Knuth, Donald E.\n  date: 1984\n";

    #[test]
    fn a_good_file_reads_entirely() {
        let reading = read_hayagriva("refs.yml", GOOD);
        assert!(reading.problems.is_empty(), "{:?}", reading.problems);
        assert!(reading.has_entries);
        assert_eq!(reading.sources[0].key, "knuth1984");
        assert_eq!(reading.sources[0].authors, vec!["Knuth D. E."]);
        assert_eq!(reading.sources[0].year, Some(1984));
        assert_eq!(reading.sources[0].line, 1);
    }

    #[test]
    fn an_entry_without_type_is_named_and_the_rest_stays() {
        let text = format!("{GOOD}notype:\n  title: Без вида\n");
        let reading = read_hayagriva("refs.yml", &text);
        assert_eq!(reading.sources.len(), 1);
        assert_eq!(reading.problems.len(), 1);
        assert_eq!(reading.problems[0].kind, SourceReadProblemKind::NoEntryType);
        assert_eq!(reading.problems[0].line, 6);
    }

    #[test]
    fn broken_yaml_is_a_file_level_problem() {
        let reading = read_hayagriva("refs.yml", "a: [\n");
        assert!(reading.sources.is_empty());
        assert!(matches!(
            reading.problems[0].kind,
            SourceReadProblemKind::FileSyntax { .. }
        ));
    }

    #[test]
    fn foreign_yaml_has_no_entries() {
        let reading = read_hayagriva("config.yml", "name: тест\nversion: 2\n");
        assert!(!reading.has_entries);
    }
}
