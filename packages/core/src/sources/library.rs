//! Библиотека источников vault: файлы, их записи и поиск (research.md §3, §14).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{
    discover::source_format, FileReading, RecognizedBy, Source, SourceFileInfo, SourceFormat,
    SourceReadProblem, SourceReadProblemKind,
};
use crate::vault::backend::VaultBackend;

/// Шаг обновления: за вызов читается один файл; `pending > 0` — вызвать ещё.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RefreshStep {
    /// Состав или содержимое библиотеки изменились.
    pub changed: bool,
    pub pending: u32,
}

/// Сводка библиотеки для панели и палитры.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LibrarySummary {
    pub files: Vec<SourceFileInfo>,
    pub source_count: u32,
    pub problems: Vec<SourceReadProblem>,
    /// YAML vault, не опознанные как источники, — для «Указать существующий…».
    pub yaml_candidates: Vec<String>,
}

/// Найденный источник. `cited` — `None`, пока индекс цитат не построен.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SourceHit {
    pub source: Source,
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub cited: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SourceHits {
    pub hits: Vec<SourceHit>,
    pub total_matches: u32,
}

/// Прочитанный файл и то, по чему решалось, источник ли он.
#[derive(Debug, Clone)]
struct LoadedFile {
    stamp: (u64, i64),
    format: SourceFormat,
    /// `None` — посторонний YAML: в сводке его нет.
    recognized: Option<RecognizedBy>,
    reading: FileReading,
    /// YAML читался как указанный или подключённый — смена этого признака
    /// требует решить заново.
    basis: (bool, bool),
}

/// Подготовленная строка поиска одного источника.
#[derive(Debug, Clone)]
struct Prepared {
    file: String,
    index: usize,
    haystack: String,
    key: String,
    /// Ключ как записан — для счётчика цитат.
    original: String,
    families: Vec<String>,
    first_author: String,
    year: i32,
}

#[derive(Debug, Clone, Default)]
pub struct SourceLibrary {
    files: BTreeMap<String, LoadedFile>,
    prepared: Vec<Prepared>,
    key_set: std::collections::HashSet<String>,
}

impl SourceLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Сверяет кандидатов с загруженным и читает **один** файл за вызов.
    /// `registered` — пути из настроек vault, `referenced` — пути из
    /// `#bibliography(…)` заметок (оба — для опознания YAML).
    pub async fn refresh<B: VaultBackend>(
        &mut self,
        backend: &B,
        registered: &BTreeSet<String>,
        referenced: &BTreeSet<String>,
    ) -> RefreshStep {
        let candidates = backend.list_source_candidates().await;
        let present: BTreeSet<&str> = candidates.iter().map(|c| c.path.as_str()).collect();
        let before = self.files.len();
        self.files.retain(|path, _| present.contains(path.as_str()));
        let mut changed = self.files.len() != before;

        let queue: Vec<_> = candidates
            .iter()
            .filter_map(|candidate| {
                let format = source_format(Path::new(&candidate.path))?;
                let basis = (
                    registered.contains(&candidate.path),
                    referenced.contains(&candidate.path),
                );
                let stamp = (candidate.size, candidate.modified_ms);
                let fresh = self.files.get(&candidate.path).is_some_and(|loaded| {
                    loaded.stamp == stamp
                        && (format == SourceFormat::BibTex || loaded.basis == basis)
                });
                (!fresh).then_some((candidate, format, basis, stamp))
            })
            .collect();
        let Some(&(candidate, format, basis, stamp)) = queue.first() else {
            if changed {
                self.prepare();
            }
            return RefreshStep {
                changed,
                pending: 0,
            };
        };

        let reading = match backend.read(Path::new(&candidate.path)).await {
            Ok(text) => match format {
                SourceFormat::BibTex => super::bibtex::read_bibtex(&candidate.path, &text),
                SourceFormat::Hayagriva => super::yaml::read_hayagriva(&candidate.path, &text),
            },
            Err(error) => FileReading {
                sources: Vec::new(),
                problems: vec![SourceReadProblem {
                    file: candidate.path.clone(),
                    line: 0,
                    column: 0,
                    key: None,
                    kind: SourceReadProblemKind::FileUnreadable {
                        reason: format!("{error:?}"),
                    },
                }],
                has_entries: false,
            },
        };
        let recognized = match format {
            SourceFormat::BibTex => Some(RecognizedBy::Extension),
            SourceFormat::Hayagriva if reading.has_entries => Some(RecognizedBy::Content),
            SourceFormat::Hayagriva if basis.0 => Some(RecognizedBy::Registered),
            SourceFormat::Hayagriva if basis.1 => Some(RecognizedBy::Referenced),
            SourceFormat::Hayagriva => None,
        };
        let previous = self.files.insert(
            candidate.path.clone(),
            LoadedFile {
                stamp,
                format,
                recognized,
                reading,
                basis,
            },
        );
        changed |= previous.is_none_or(|old| {
            old.recognized != recognized || old.reading != self.files[&candidate.path].reading
        });
        if changed {
            self.prepare();
        }
        RefreshStep {
            changed,
            pending: queue.len() as u32 - 1,
        }
    }

    pub fn summary(&self) -> LibrarySummary {
        let mut files = Vec::new();
        let mut problems = Vec::new();
        let mut yaml_candidates = Vec::new();
        let mut source_count = 0;
        for (path, loaded) in &self.files {
            match loaded.recognized {
                Some(recognized_by) => {
                    source_count += loaded.reading.sources.len() as u32;
                    problems.extend(loaded.reading.problems.iter().cloned());
                    files.push(SourceFileInfo {
                        path: path.clone(),
                        format: loaded.format,
                        source_count: loaded.reading.sources.len() as u32,
                        problem_count: loaded.reading.problems.len() as u32,
                        recognized_by,
                    });
                }
                None => yaml_candidates.push(path.clone()),
            }
        }
        LibrarySummary {
            files,
            source_count,
            problems,
            yaml_candidates,
        }
    }

    /// Все ключи источников — для проверки цитат.
    pub fn has_key(&self, key: &str) -> bool {
        self.key_set.contains(key)
    }

    /// Есть ли в vault файлы источников (для правила отметок, research.md §10).
    pub fn has_files(&self) -> bool {
        self.files
            .values()
            .any(|loaded| loaded.recognized.is_some())
    }

    /// Источник по файлу и ключу.
    pub fn source(&self, file: &str, key: &str) -> Option<&Source> {
        let loaded = self.files.get(file)?;
        loaded.recognized?;
        loaded
            .reading
            .sources
            .iter()
            .find(|source| source.key == key)
    }

    pub fn keys(&self) -> BTreeSet<String> {
        self.sources().map(|source| source.key.clone()).collect()
    }

    fn sources(&self) -> impl Iterator<Item = &Source> {
        self.files
            .values()
            .filter(|loaded| loaded.recognized.is_some())
            .flat_map(|loaded| loaded.reading.sources.iter())
    }

    /// Поиск по авторам, названию, году и ключу (research.md §14). Каждое
    /// слово запроса обязано встретиться; `cited` даёт число цитат ключа
    /// (`None` — индекс не готов); `uncited_only` оставляет только
    /// непроцитированные, если `cited` готов.
    pub fn search(
        &self,
        query: &str,
        limit: u32,
        cited: Option<&dyn Fn(&str) -> u32>,
        uncited_only: bool,
    ) -> SourceHits {
        // «Не цитируется» по неполному индексу было бы неправдой.
        if uncited_only && cited.is_none() {
            return SourceHits {
                hits: Vec::new(),
                total_matches: 0,
            };
        }
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let first = words.first().cloned().unwrap_or_default();
        let mut matched: Vec<(u8, &Prepared)> = self
            .prepared
            .iter()
            .filter(|prepared| {
                words
                    .iter()
                    .all(|word| prepared.haystack.contains(word.as_str()))
            })
            .filter(|prepared| {
                !uncited_only || cited.is_some_and(|count| count(&prepared.original) == 0)
            })
            .map(|prepared| {
                let rank = if first.is_empty() {
                    2
                } else if prepared.key.starts_with(&first) {
                    0
                } else if prepared
                    .families
                    .iter()
                    .any(|family| family.starts_with(&first))
                {
                    1
                } else {
                    2
                };
                (rank, prepared)
            })
            .collect();
        matched.sort_by(|(left_rank, left), (right_rank, right)| {
            left_rank
                .cmp(right_rank)
                .then_with(|| left.first_author.cmp(&right.first_author))
                .then_with(|| left.year.cmp(&right.year))
                .then_with(|| left.key.cmp(&right.key))
        });
        let total_matches = matched.len() as u32;
        let hits = matched
            .into_iter()
            .take(limit as usize)
            .filter_map(|(_, prepared)| {
                let source = self
                    .files
                    .get(&prepared.file)?
                    .reading
                    .sources
                    .get(prepared.index)?;
                Some(SourceHit {
                    cited: cited.map(|count| count(&source.key)),
                    source: source.clone(),
                })
            })
            .collect();
        SourceHits {
            hits,
            total_matches,
        }
    }

    /// Подготовленные строки поиска — после каждого изменения состава.
    fn prepare(&mut self) {
        self.key_set = self.sources().map(|source| source.key.clone()).collect();
        self.prepared = self
            .files
            .iter()
            .filter(|(_, loaded)| loaded.recognized.is_some())
            .flat_map(|(path, loaded)| {
                loaded
                    .reading
                    .sources
                    .iter()
                    .enumerate()
                    .map(move |(index, source)| Prepared {
                        file: path.clone(),
                        index,
                        haystack: format!(
                            "{} {} {} {}",
                            source.authors.join(" "),
                            source.title,
                            source.year.map(|year| year.to_string()).unwrap_or_default(),
                            source.key
                        )
                        .to_lowercase(),
                        key: source.key.to_lowercase(),
                        original: source.key.clone(),
                        families: source
                            .authors
                            .iter()
                            .map(|author| author.to_lowercase())
                            .collect(),
                        first_author: source
                            .authors
                            .first()
                            .map(|author| author.to_lowercase())
                            .unwrap_or_default(),
                        year: source.year.unwrap_or(i32::MAX),
                    })
            })
            .collect();
    }
}
