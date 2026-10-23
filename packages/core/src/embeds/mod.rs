//! Встраивание заметок (фича 031).
//!
//! Встраивание — вызов `#embed("цель")` из библиотеки `.typsidian/lib/embed.typ`
//! (research.md §1). Цель — `заметка`, `заметка#Заголовок` или
//! `заметка#<метка>`; путь заметки — по правилам ссылок `[[…]]` (013).
//!
//! Перед сборкой [`plan`] находит вызовы, вырезает встраиваемое и отдаёт его
//! сборке виртуальными файлами рядом с исходной заметкой, а соответствие цели
//! и файла — входом `sys.inputs["typsidian-embed"]` (research.md §2). Текст
//! открытой заметки не меняется. Вызовы внутри встроенного текста ядро
//! переписывает на свои ключи: цикл и глубина считаются по цепочке каждого
//! вхождения.

mod library;
#[cfg(target_arch = "wasm32")]
pub mod web;

use std::ops::Range;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use typst::syntax::{LinkedNode, SyntaxKind};

use crate::graph::resolve_link_target;
use crate::vault::backend::VaultBackend;

pub use library::{
    embed_import_edits, embed_library, ensure_embed_library, insert_embed, EmbedInsertion,
    LIBRARY_IMPORT, LIBRARY_PATH, LIBRARY_TEXT,
};

/// Ключ входа сборки.
pub const EMBED_INPUT_KEY: &str = "typsidian-embed";
/// Глубина вложенных встраиваний (Edge Cases).
pub const MAX_DEPTH: usize = 5;
/// Начало ключа вызова внутри встроенного текста — не бывает в именах заметок.
const NESTED_KEY: char = '\u{1}';

// ── Цель ────────────────────────────────────────────────────────────────────

/// Что встраивается из заметки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind", content = "name")]
pub enum EmbedPart {
    Whole,
    /// Раздел по тексту заголовка.
    Section(String),
    /// Элемент по метке, имя без `<>`.
    Label(String),
}

/// Разобранная цель встраивания.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedTarget {
    /// Путь заметки как написан.
    pub note: String,
    pub part: EmbedPart,
}

/// `заметка`, `заметка#Заголовок`, `заметка#<метка>`.
pub fn parse_target(raw: &str) -> EmbedTarget {
    let (note, part) = match raw.split_once('#') {
        None => (raw, EmbedPart::Whole),
        Some((note, rest)) => {
            let rest = rest.trim();
            let part = match rest.strip_prefix('<').and_then(|r| r.strip_suffix('>')) {
                Some(label) => EmbedPart::Label(label.trim().to_owned()),
                None if rest.is_empty() => EmbedPart::Whole,
                None => EmbedPart::Section(rest.to_owned()),
            };
            (note, part)
        }
    };
    EmbedTarget {
        note: note.trim().to_owned(),
        part,
    }
}

// ── Вызовы в тексте ─────────────────────────────────────────────────────────

/// Вызов `embed("…")` в тексте.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedCall {
    /// Строка аргумента как написана.
    pub raw: String,
    /// Байты строки внутри кавычек.
    pub string: Range<usize>,
    /// Весь вызов, с `#`, если он есть.
    pub call: Range<usize>,
}

fn ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-') || byte >= 0x80
}

/// Вызовы `embed("…")` по порядку текста. Разбор — по байтам, как у
/// `[[…]]` (`graph::link_sites`): строка без экранирования и переводов строки,
/// сразу за ней `)`.
pub fn calls(text: &str) -> Vec<EmbedCall> {
    const OPEN: &str = "embed(\"";
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find(OPEN) {
        let at = from + found;
        from = at + OPEN.len();
        if at > 0 && ident_byte(bytes[at - 1]) {
            continue;
        }
        let start = at + OPEN.len();
        let Some(len) = text[start..].find(['"', '\n', '\\']) else {
            break;
        };
        let end = start + len;
        if bytes[end] != b'"' {
            continue;
        }
        let after = &text[end + 1..];
        let spaces = after.len() - after.trim_start_matches([' ', '\t']).len();
        if !after[spaces..].starts_with(')') {
            continue;
        }
        let call_start = if at > 0 && bytes[at - 1] == b'#' {
            at - 1
        } else {
            at
        };
        out.push(EmbedCall {
            raw: text[start..end].to_owned(),
            string: start..end,
            call: call_start..end + 1 + spaces + 1,
        });
        from = end + 1;
    }
    out
}

// ── Вырезание ───────────────────────────────────────────────────────────────

/// Текст заголовка без `=` и метки — так его пишут в цели.
fn heading_title(node: &LinkedNode, text: &str) -> String {
    node.children()
        .filter(|child| !matches!(child.kind(), SyntaxKind::HeadingMarker | SyntaxKind::Label))
        .map(|child| &text[child.range()])
        .collect::<String>()
        .trim()
        .to_owned()
}

fn heading_level(node: &LinkedNode) -> usize {
    node.children()
        .find(|child| child.kind() == SyntaxKind::HeadingMarker)
        .map_or(1, |marker| marker.get().full_text().len())
}

fn headings<'a>(node: LinkedNode<'a>, out: &mut Vec<LinkedNode<'a>>) {
    if node.kind() == SyntaxKind::Heading {
        out.push(node);
        return;
    }
    if matches!(node.kind(), SyntaxKind::Raw | SyntaxKind::Equation) {
        return;
    }
    for child in node.children() {
        headings(child, out);
    }
}

fn label_node<'a>(node: LinkedNode<'a>, name: &str) -> Option<LinkedNode<'a>> {
    if node.kind() == SyntaxKind::Label && node.get().full_text().trim_matches(['<', '>']) == name {
        return Some(node);
    }
    if matches!(node.kind(), SyntaxKind::Raw | SyntaxKind::Equation) {
        return None;
    }
    node.children().find_map(|child| label_node(child, name))
}

/// Встраиваемый текст заметки; `None` — нет такого раздела или метки.
pub fn extract(text: &str, part: &EmbedPart) -> Option<String> {
    extract_in(&typst::syntax::parse(text), text, part)
}

/// [`extract`] над уже разобранным текстом: план разбирает заметку один раз,
/// сколько бы её частей ни встраивалось.
fn extract_in(root: &typst::syntax::SyntaxNode, text: &str, part: &EmbedPart) -> Option<String> {
    let linked = LinkedNode::new(root);
    match part {
        EmbedPart::Whole => Some(text.to_owned()),
        EmbedPart::Section(title) => {
            let mut found = Vec::new();
            headings(linked, &mut found);
            let index = found
                .iter()
                .position(|heading| heading_title(heading, text) == title.trim())?;
            let level = heading_level(&found[index]);
            let start = found[index].range().start;
            let end = found[index + 1..]
                .iter()
                .find(|next| heading_level(next) <= level)
                .map_or(text.len(), |next| next.range().start);
            Some(text[start..end].trim_end().to_owned())
        }
        EmbedPart::Label(name) => {
            let label = label_node(linked, name)?;
            let label_range = label.range();
            // Метка заголовка — сам заголовок без метки.
            if let Some(heading) = label
                .parent()
                .filter(|parent| parent.kind() == SyntaxKind::Heading)
            {
                let range = heading.range();
                let mut out = text[range.start..label_range.start].to_owned();
                out.push_str(&text[label_range.end..range.end]);
                return Some(out.trim_end().to_owned());
            }
            // Иначе — предыдущий значимый сосед: формула, вызов, блок…
            let mut previous = label.prev_sibling();
            while let Some(node) = previous.as_ref() {
                if matches!(node.kind(), SyntaxKind::Space) {
                    previous = node.prev_sibling();
                } else {
                    break;
                }
            }
            let start = match previous {
                Some(node)
                    if matches!(
                        node.kind(),
                        SyntaxKind::Equation
                            | SyntaxKind::FuncCall
                            | SyntaxKind::Raw
                            | SyntaxKind::ContentBlock
                            | SyntaxKind::Heading
                            | SyntaxKind::ListItem
                            | SyntaxKind::EnumItem
                    ) =>
                {
                    let start = node.range().start;
                    if start > 0 && text.as_bytes()[start - 1] == b'#' {
                        start - 1
                    } else {
                        start
                    }
                }
                // Абзац: от предыдущей пустой строки.
                _ => text[..label_range.start]
                    .rfind("\n\n")
                    .map_or(0, |at| at + 2),
            };
            Some(text[start..label_range.start].trim().to_owned())
        }
    }
}

// ── План сборки ─────────────────────────────────────────────────────────────

/// Что сборка получает от встраиваний заметки (data-model.md).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmbedPlan {
    /// JSON входа `typsidian-embed`; `None` — вызовов нет.
    pub inputs: Option<String>,
    /// Виртуальные файлы: путь vault и текст.
    pub files: Vec<(String, String)>,
    /// Встроенные заметки — зависимости документа (FR-003).
    pub sources: Vec<String>,
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Путь виртуального файла встройки — рядом с заметкой, чтобы относительные
/// пути внутри встроенного текста работали как в самой заметке.
fn virtual_path(note: &Path, index: usize) -> String {
    let stem = note
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = format!("{stem}.embed-{index}.typ");
    match note
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        Some(parent) => format!("{}/{name}", slashed(parent)),
        None => name,
    }
}

/// Одно вхождение встраивания, которое осталось разобрать.
struct Job {
    /// Ключ ответа: строка цели в открытой заметке или `\u{1}<n>`.
    key: String,
    raw: String,
    /// Заметка, в которой стоит вызов.
    source: std::path::PathBuf,
    /// Цепочка встраиваний до этого вызова, пути vault.
    chain: Vec<String>,
}

/// Ответ-ошибка для цели.
fn failure(error: &str, target: &str, extra: Value) -> Value {
    let mut entry = json!({ "error": error, "target": target });
    if let (Value::Object(entry), Value::Object(extra)) = (&mut entry, extra) {
        entry.extend(extra);
    }
    entry
}

/// План встраиваний открытой заметки `note_path` с текстом `text`.
///
/// Без рекурсии: тексту родителя нужен только ключ вложенного вызова, а не
/// путь его встройки, поэтому вложенные цели ставятся в очередь и
/// разбираются после — со своей цепочкой (цикл, глубина).
pub async fn plan<B: VaultBackend>(backend: &B, note_path: &str, text: &str) -> EmbedPlan {
    let found = calls(text);
    if found.is_empty() {
        return EmbedPlan::default();
    }
    let main = Path::new(note_path);
    let mut entries = Map::new();
    let mut files: Vec<(String, String)> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    let mut keys = 0usize;
    let mut queue: std::collections::VecDeque<Job> = std::collections::VecDeque::new();
    // Прочитанные и разобранные цели: заметку читают и разбирают один раз.
    let mut parsed: std::collections::HashMap<String, Option<(String, typst::syntax::SyntaxNode)>> =
        std::collections::HashMap::new();
    for call in found {
        if queue.iter().any(|job| job.key == call.raw) {
            continue;
        }
        queue.push_back(Job {
            key: call.raw.clone(),
            raw: call.raw,
            source: main.to_path_buf(),
            chain: vec![slashed(main)],
        });
    }

    while let Some(job) = queue.pop_front() {
        let target = parse_target(&job.raw);
        let Some(path) = resolve_link_target(&job.source, &target.note) else {
            entries.insert(job.key, failure("note", &target.note, json!({})));
            continue;
        };
        if path.extension().and_then(|ext| ext.to_str()) == Some(crate::vault::BOARD_EXTENSION) {
            entries.insert(job.key, failure("board", &target.note, json!({})));
            continue;
        }
        let path_text = slashed(&path);
        if job.chain.contains(&path_text) {
            let mut names: Vec<String> = job
                .chain
                .iter()
                .map(|note| crate::links::title_of(note))
                .collect();
            names.push(crate::links::title_of(&path_text));
            entries.insert(
                job.key,
                failure("cycle", &target.note, json!({ "chain": names })),
            );
            continue;
        }
        if job.chain.len() > MAX_DEPTH {
            entries.insert(job.key, failure("depth", &target.note, json!({})));
            continue;
        }
        if !sources.contains(&path_text) {
            sources.push(path_text.clone());
        }
        if !parsed.contains_key(&path_text) {
            let read = backend.read(&path).await.ok().map(|text| {
                let root = typst::syntax::parse(&text);
                (text, root)
            });
            parsed.insert(path_text.clone(), read);
        }
        let Some((note_text, root)) = parsed.get(&path_text).and_then(Option::as_ref) else {
            entries.insert(job.key, failure("note", &target.note, json!({})));
            continue;
        };
        let Some(part) = extract_in(root, note_text, &target.part) else {
            let name = match &target.part {
                EmbedPart::Section(name) => name.clone(),
                EmbedPart::Label(name) => format!("<{name}>"),
                EmbedPart::Whole => String::new(),
            };
            entries.insert(
                job.key,
                failure("part", &target.note, json!({ "part": name })),
            );
            continue;
        };

        // Вызовы внутри встроенного текста — на свои ключи, со своей цепочкой.
        let nested = calls(&part);
        let rewritten = if nested.is_empty() {
            part
        } else {
            let mut chain = job.chain.clone();
            chain.push(path_text.clone());
            let mut out = String::with_capacity(part.len() + 64);
            // Своя строка `#import`: раздел мог оказаться без неё.
            out.push_str(&format!("#import \"{LIBRARY_IMPORT}\": embed\n"));
            let mut cursor = 0;
            for call in nested {
                keys += 1;
                out.push_str(&part[cursor..call.string.start]);
                out.push_str(&format!("\\u{{1}}{keys}"));
                cursor = call.string.end;
                queue.push_back(Job {
                    key: format!("{NESTED_KEY}{keys}"),
                    raw: call.raw,
                    source: path.clone(),
                    chain: chain.clone(),
                });
            }
            out.push_str(&part[cursor..]);
            out
        };
        let file = virtual_path(&path, files.len());
        entries.insert(job.key, json!({ "path": format!("/{file}") }));
        files.push((file, rewritten));
    }

    EmbedPlan {
        inputs: serde_json::to_string(&Value::Object(entries)).ok(),
        files,
        sources,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_split_note_and_part() {
        assert_eq!(parse_target("Методика").part, EmbedPart::Whole);
        assert_eq!(
            parse_target(" /Справка/Метрология # Выборка ").part,
            EmbedPart::Section("Выборка".into())
        );
        assert_eq!(
            parse_target("Методика#<eq-fourier>").part,
            EmbedPart::Label("eq-fourier".into())
        );
        assert_eq!(parse_target("Методика#Выборка").note, "Методика");
    }

    #[test]
    fn calls_are_found_with_their_ranges() {
        let text =
            "a #embed(\"Методика#Выборка\") b embed(\"x\" ) myembed(\"no\") #embed(\"z\", 1)";
        let found = calls(text);
        let raws: Vec<&str> = found.iter().map(|call| call.raw.as_str()).collect();
        assert_eq!(raws, ["Методика#Выборка", "x"]);
        assert_eq!(&text[found[0].call.clone()], "#embed(\"Методика#Выборка\")");
        assert_eq!(&text[found[1].call.clone()], "embed(\"x\" )");
    }
}
