//! Какие файлы — файлы источников (FR-001, research.md §3, §11).

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use super::SourceFormat;
use crate::vault::{classify, FileKind};

/// Формат кандидата по пути; `None` — не кандидат. Кандидат — расширение
/// `bib`/`yml`/`yaml` без учёта регистра у файла вида `Other`: служебное
/// место, снимки и конфликтные копии кандидатами не бывают (правило 1).
pub fn source_format(path: &Path) -> Option<SourceFormat> {
    let conflict_copy = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(crate::sync::conflict_copy::is_conflict_copy);
    if classify(path) != FileKind::Other || conflict_copy {
        return None;
    }
    let extension = path.extension()?.to_str()?;
    if extension.eq_ignore_ascii_case("bib") {
        Some(SourceFormat::BibTex)
    } else if extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml") {
        Some(SourceFormat::Hayagriva)
    } else {
        None
    }
}

/// Кандидат ли путь — предикат для обходов бэкендов.
pub fn is_source_candidate(path: &Path) -> bool {
    source_format(path).is_some()
}

/// Почему путь нельзя указать файлом источников.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum SourceFileProblem {
    #[error("путь не указан")]
    Empty,
    #[error("файл должен лежать внутри vault")]
    OutsideVault,
    #[error("файл источников — .bib, .yml или .yaml")]
    WrongExtension,
}

/// Нормальная форма пути файла источников из настроек (research.md §11).
pub fn validate_source_file(raw: &str) -> Result<String, SourceFileProblem> {
    let trimmed = raw.trim().replace('\\', "/");
    let trimmed = trimmed.trim_start_matches('/');
    if trimmed.is_empty() {
        return Err(SourceFileProblem::Empty);
    }
    let path = Path::new(trimmed);
    if path
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
        || trimmed.contains(':')
    {
        return Err(SourceFileProblem::OutsideVault);
    }
    if source_format(path).is_none() {
        return Err(SourceFileProblem::WrongExtension);
    }
    Ok(trimmed
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_by_extension_regardless_of_case() {
        assert_eq!(
            source_format(Path::new("a.bib")),
            Some(SourceFormat::BibTex)
        );
        assert_eq!(
            source_format(Path::new("x/A.BIB")),
            Some(SourceFormat::BibTex)
        );
        assert_eq!(
            source_format(Path::new("s.yml")),
            Some(SourceFormat::Hayagriva)
        );
        assert_eq!(
            source_format(Path::new("s.YAML")),
            Some(SourceFormat::Hayagriva)
        );
        assert_eq!(source_format(Path::new("note.typ")), None);
    }

    #[test]
    fn service_dir_snapshots_and_conflict_copies_are_not_candidates() {
        assert_eq!(source_format(Path::new(".typsidian/a.bib")), None);
        assert_eq!(
            source_format(Path::new("refs.sync-conflict-20261003-120000-ABCDEFG.bib")),
            None
        );
    }

    #[test]
    fn registered_paths_are_normalized_or_refused() {
        assert_eq!(
            validate_source_file(" /lit/refs.yml "),
            Ok("lit/refs.yml".into())
        );
        assert_eq!(validate_source_file(""), Err(SourceFileProblem::Empty));
        assert_eq!(
            validate_source_file("../refs.bib"),
            Err(SourceFileProblem::OutsideVault)
        );
        assert_eq!(
            validate_source_file("C:\\refs.bib"),
            Err(SourceFileProblem::OutsideVault)
        );
        assert_eq!(
            validate_source_file("refs.txt"),
            Err(SourceFileProblem::WrongExtension)
        );
    }
}
