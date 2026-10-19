//! Запись цитаты и предложение списка литературы (FR-003, FR-004,
//! research.md §12, §13).
//!
//! Допустимость знаков ключа — функциями лексера `typst::syntax`, а не своим
//! списком: краткая запись `@ключ` обязана читаться ровно так, как её прочтёт
//! сборка.

use std::path::Path;

use serde::{Deserialize, Serialize};
use typst::syntax::{is_id_continue, is_valid_label_literal_id, Lines};

use crate::elements::edit::TextEdit;
use crate::structure::{mode_at, parse, TextMode};

/// Правка заметки для вставки цитаты: замены одной транзакцией и место
/// каретки после неё (UTF-16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CitationPlan {
    pub edits: Vec<TextEdit>,
    pub caret: usize,
}

/// Почему цитата не вставлена.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum CitationPlanError {
    /// Каретка в строке или комментарии: цитата там не будет цитатой.
    NotInsertable { mode: TextMode },
    /// Источника с таким ключом в файле нет (файл изменился).
    UnknownKey,
}

/// Предложение подключить файл источников к заметке (FR-004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum BibliographyOffer {
    None,
    /// В заметке нет списка литературы — дописать его в конец.
    Add {
        file: String,
        edits: Vec<TextEdit>,
    },
    /// Список есть, но этого файла в нём нет — дополнить аргумент.
    Extend {
        file: String,
        edits: Vec<TextEdit>,
    },
}

/// Запись цитаты для режима и знака за кареткой (research.md §12). `None` —
/// в строке и комментарии цитату не вставить.
pub fn citation_text(
    key: &str,
    mode: TextMode,
    previous: Option<char>,
    next: Option<char>,
    after_next: Option<char>,
) -> Option<String> {
    let label_ok = is_valid_label_literal_id(key);
    let call = |hash: &str| {
        if label_ok {
            format!("{hash}cite(<{key}>)")
        } else {
            format!("{hash}cite(label({}))", quoted(key))
        }
    };
    match mode {
        TextMode::String | TextMode::Comment => None,
        TextMode::Code => Some(call("")),
        TextMode::Math => Some(call("#")),
        TextMode::Markup => {
            // Краткая запись прочиталась бы иначе: имя продолжилось бы
            // следующим знаком, `[` стал бы дополнением, хвостовые `.`/`:`
            // лексер отрезает, а `@` сразу за словом — не ссылка.
            let continues = next.is_some_and(|c| {
                is_id_continue(c)
                    || c == '['
                    || (matches!(c, '.' | ':') && after_next.is_some_and(is_id_continue))
            });
            let glued = previous.is_some_and(|c| c.is_alphanumeric() || c == '_');
            let short = label_ok && !key.ends_with(['.', ':']) && !continues && !glued;
            Some(if short { format!("@{key}") } else { call("#") })
        }
    }
}

/// Цитата в месте каретки (`caret` — UTF-16). `known` — ключ есть в
/// библиотеке: вставлять цитату на ключ, которого нет, бессмысленно.
pub fn plan_citation(
    text: &str,
    caret: usize,
    key: &str,
    known: bool,
) -> Result<CitationPlan, CitationPlanError> {
    if !known {
        return Err(CitationPlanError::UnknownKey);
    }
    let lines = Lines::new(text);
    let caret = caret.min(lines.len_utf16());
    let mode = mode_at(text, caret);
    let byte = lines.utf16_to_byte(caret).unwrap_or(text.len());
    let previous = text[..byte].chars().next_back();
    let mut after = text[byte..].chars();
    let next = after.next();
    let after_next = after.next();
    let insert = citation_text(key, mode, previous, next, after_next)
        .ok_or(CitationPlanError::NotInsertable { mode })?;
    let length = insert.encode_utf16().count();
    Ok(CitationPlan {
        edits: vec![TextEdit {
            from: caret,
            to: caret,
            insert,
        }],
        caret: caret + length,
    })
}

/// Предложение подключить файл `file` (путь от корня vault) к заметке
/// `note_path` (research.md §13). Путь в записи — от корня с ведущей `/`: он
/// не ломается при переносе заметки.
pub fn bibliography_offer(text: &str, note_path: &str, file: &str) -> BibliographyOffer {
    let structure = parse(text);
    let rooted = format!("/{}", file.trim_start_matches('/'));
    let Some(site) = structure.bibliographies.first() else {
        let end = text.encode_utf16().count();
        let lead = if text.is_empty() || text.ends_with("\n\n") {
            ""
        } else if text.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        return BibliographyOffer::Add {
            file: file.to_owned(),
            edits: vec![TextEdit {
                from: end,
                to: end,
                insert: format!("{lead}#bibliography({})\n", quoted(&rooted)),
            }],
        };
    };
    if !site.literal {
        return BibliographyOffer::None;
    }
    let folder = Path::new(note_path).parent().unwrap_or(Path::new(""));
    let named = site
        .paths
        .iter()
        .any(|path| resolve(folder, path) == file.trim_start_matches('/'));
    if named {
        return BibliographyOffer::None;
    }
    let mut items: Vec<String> = site.paths.iter().map(|path| quoted(path)).collect();
    items.push(quoted(&rooted));
    BibliographyOffer::Extend {
        file: file.to_owned(),
        edits: vec![TextEdit {
            from: site.arg.from as usize,
            to: site.arg.to as usize,
            insert: format!("({})", items.join(", ")),
        }],
    }
}

/// Путь из `bibliography(…)` от корня vault — так же, как у сборки: `/…` —
/// от корня, иначе от папки заметки.
pub(crate) fn resolve(folder: &Path, path: &str) -> String {
    let joined = match path.strip_prefix('/') {
        Some(rooted) => Path::new(rooted).to_path_buf(),
        None => folder.join(path),
    };
    let mut parts: Vec<String> = Vec::new();
    for component in joined.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            std::path::Component::ParentDir => {
                parts.pop();
            }
            _ => {}
        }
    }
    parts.join("/")
}

/// Строка Typst в кавычках с экранированием.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_form_where_it_reads_back() {
        assert_eq!(
            citation_text("knuth1984", TextMode::Markup, Some(' '), Some(' '), None).as_deref(),
            Some("@knuth1984")
        );
        assert_eq!(
            citation_text("knuth1984", TextMode::Markup, None, Some('a'), None).as_deref(),
            Some("#cite(<knuth1984>)")
        );
        assert_eq!(
            citation_text("key.", TextMode::Markup, None, None, None).as_deref(),
            Some("#cite(<key.>)")
        );
        assert_eq!(
            citation_text("с пробелом", TextMode::Markup, None, None, None).as_deref(),
            Some("#cite(label(\"с пробелом\"))")
        );
        assert_eq!(
            citation_text("k", TextMode::Code, None, None, None).as_deref(),
            Some("cite(<k>)")
        );
        assert_eq!(
            citation_text("k", TextMode::Comment, None, None, None),
            None
        );
    }

    #[test]
    fn offer_adds_extends_or_stays_quiet() {
        assert!(matches!(
            bibliography_offer("= Глава\n", "a/Глава.typ", "refs.bib"),
            BibliographyOffer::Add { .. }
        ));
        let named = "#bibliography(\"../refs.bib\")\n";
        assert_eq!(
            bibliography_offer(named, "a/Глава.typ", "refs.bib"),
            BibliographyOffer::None
        );
        match bibliography_offer("#bibliography(\"/other.bib\")\n", "a/x.typ", "refs.bib") {
            BibliographyOffer::Extend { edits, .. } => {
                assert_eq!(edits[0].insert, "(\"/other.bib\", \"/refs.bib\")")
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            bibliography_offer(
                "#let f = \"x.bib\"\n#bibliography(f)\n",
                "x.typ",
                "refs.bib"
            ),
            BibliographyOffer::None
        );
    }
}
