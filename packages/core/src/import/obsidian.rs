//! Особенности Obsidian поверх CommonMark (фича 021, research.md §3).
//!
//! Чистые функции над строками: YAML-шапка, цель вики-ссылки, размер картинки,
//! заголовок выноски. Связь с Typst — только запись свойств 020 через
//! `properties::write`, чтобы формат записи оставался в одном месте.

use crate::properties::write::{render_key, render_value};
use crate::properties::PropertyValue;

/// Шапка и остаток заметки.
#[derive(Debug, PartialEq, Eq)]
pub struct Split<'a> {
    /// Текст YAML без ограничителей `---`.
    pub yaml: Option<&'a str>,
    /// Остаток заметки после шапки.
    pub body: &'a str,
    /// Сколько строк исходного файла заняла шапка (для номеров строк потерь).
    pub lines: usize,
}

/// Отделяет YAML-шапку: файл начинается с `---` отдельной строкой и шапка
/// закрыта `---` (или `...`) отдельной строкой. Иначе шапки нет.
pub fn split_frontmatter(source: &str) -> Split<'_> {
    let none = Split {
        yaml: None,
        body: source,
        lines: 0,
    };
    let Some(rest) = source
        .strip_prefix("---\n")
        .or_else(|| source.strip_prefix("---\r\n"))
    else {
        return none;
    };
    let mut offset = 0;
    for (index, line) in rest.split_inclusive('\n').enumerate() {
        let lines = index + 2;
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" || trimmed == "..." {
            return Split {
                yaml: Some(&rest[..offset]),
                body: &rest[offset + line.len()..],
                lines,
            };
        }
        offset += line.len();
    }
    none
}

/// Свойство из шапки.
pub type Entry = (String, PropertyValue);

/// Разбирает плоский YAML: скаляры, строки в кавычках, списки в `[a, b]` и
/// блоком `- a`, блочные скаляры `|` и `>`. Вложенные словари и якоря не
/// поддерживаются — тогда `None`, и шапка остаётся комментарием.
pub fn parse_yaml(yaml: &str) -> Option<Vec<Entry>> {
    let lines: Vec<&str> = yaml.lines().collect();
    let mut entries: Vec<Entry> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with([' ', '\t']) || line.starts_with("- ") {
            // Продолжение, которое не принадлежит ни одному ключу.
            return None;
        }
        let (key, rest) = line.split_once(':')?;
        let key = unquote(key.trim())?;
        if key.is_empty() {
            return None;
        }
        let rest = strip_comment(rest).trim().to_owned();

        let value = if rest.is_empty() {
            // Либо блочный список, либо пустое значение.
            let mut items = Vec::new();
            while i < lines.len() {
                let next = lines[i];
                let trimmed = next.trim_start();
                if next.trim().is_empty() {
                    i += 1;
                    continue;
                }
                if !(next.starts_with([' ', '\t']) || next.starts_with("- ")) {
                    break;
                }
                let item = trimmed.strip_prefix("- ").or_else(|| {
                    // «-» без значения.
                    (trimmed == "-").then_some("")
                });
                items.push(scalar_text(strip_comment(item?).trim())?);
                i += 1;
            }
            if items.is_empty() {
                PropertyValue::text("")
            } else {
                PropertyValue::list(items.into_iter().map(strip_hash_for_tags(&key)))
            }
        } else if rest == "|" || rest == ">" || rest.starts_with("|-") || rest.starts_with(">-") {
            let folded = rest.starts_with('>');
            let mut block = Vec::new();
            while i < lines.len()
                && (lines[i].starts_with([' ', '\t']) || lines[i].trim().is_empty())
            {
                block.push(lines[i].trim());
                i += 1;
            }
            let joined = block.join(if folded { " " } else { "\n" });
            PropertyValue::text(joined.trim().to_owned())
        } else if rest.starts_with('[') {
            let inner = rest.strip_prefix('[')?.strip_suffix(']')?;
            let mut values = Vec::new();
            for part in split_inline(inner)? {
                if !part.trim().is_empty() {
                    values.push(scalar_text(part.trim())?);
                }
            }
            PropertyValue::list(values.into_iter().map(strip_hash_for_tags(&key)))
        } else if rest.starts_with('{') || rest.starts_with('&') || rest.starts_with('*') {
            return None;
        } else {
            scalar_value(&key, &rest)?
        };
        if !entries.iter().any(|(existing, _)| *existing == key) {
            entries.push((key, value));
        }
    }
    Some(entries)
}

/// У тегов `#` в начале необязателен: `#проект` и `проект` — один тег.
fn strip_hash_for_tags(key: &str) -> impl Fn(String) -> String {
    let is_tags = key == "tags" || key == "tag";
    move |value| {
        if is_tags {
            value.trim_start_matches('#').to_owned()
        } else {
            value
        }
    }
}

/// Обычный (не списочный) скаляр: тип по виду.
fn scalar_value(key: &str, raw: &str) -> Option<PropertyValue> {
    if raw.starts_with('"') || raw.starts_with('\'') {
        return Some(PropertyValue::text(unquote(raw)?));
    }
    if key == "tags" || key == "tag" {
        let items = raw
            .split([',', ' '])
            .map(|tag| tag.trim().trim_start_matches('#'))
            .filter(|tag| !tag.is_empty())
            .map(str::to_owned);
        return Some(PropertyValue::list(items));
    }
    match raw {
        "true" | "True" => return Some(PropertyValue::Flag { value: true }),
        "false" | "False" => return Some(PropertyValue::Flag { value: false }),
        "null" | "~" => return Some(PropertyValue::text("")),
        _ => {}
    }
    if let Some(date) = parse_date(raw) {
        return Some(date);
    }
    if is_number(raw) {
        if let Ok(value) = raw.parse::<f64>() {
            return Some(PropertyValue::Number { value });
        }
    }
    Some(PropertyValue::text(raw))
}

fn is_number(raw: &str) -> bool {
    let digits = raw.strip_prefix('-').unwrap_or(raw);
    let mut parts = digits.splitn(2, '.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next();
    !whole.is_empty()
        && whole.chars().all(|ch| ch.is_ascii_digit())
        && (whole == "0" || !whole.starts_with('0'))
        && fraction.is_none_or(|f| !f.is_empty() && f.chars().all(|ch| ch.is_ascii_digit()))
}

/// `2024-05-01` — календарная дата без времени.
fn parse_date(raw: &str) -> Option<PropertyValue> {
    let mut parts = raw.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }
    let value = PropertyValue::date(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?);
    value.is_writable().then_some(value)
}

/// Скаляр списка как текст: кавычки сняты.
fn scalar_text(raw: &str) -> Option<String> {
    if raw.starts_with('"') || raw.starts_with('\'') {
        unquote(raw)
    } else if raw.starts_with('[') || raw.starts_with('{') {
        None
    } else {
        Some(raw.to_owned())
    }
}

/// Снимает кавычки; внутри двойных понимает `\"` и `\\`.
fn unquote(raw: &str) -> Option<String> {
    let mut chars = raw.chars();
    match chars.next() {
        Some('"') => {
            let inner = raw.strip_prefix('"')?.strip_suffix('"')?;
            let mut out = String::new();
            let mut escaped = false;
            for ch in inner.chars() {
                if escaped {
                    out.push(match ch {
                        'n' => '\n',
                        't' => '\t',
                        other => other,
                    });
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else {
                    out.push(ch);
                }
            }
            Some(out)
        }
        Some('\'') => Some(
            raw.strip_prefix('\'')?
                .strip_suffix('\'')?
                .replace("''", "'"),
        ),
        _ => Some(raw.to_owned()),
    }
}

/// Отрезает ` # комментарий` вне кавычек.
fn strip_comment(raw: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut previous = ' ';
    for (index, ch) in raw.char_indices() {
        match (quote, ch) {
            (None, '"' | '\'') => quote = Some(ch),
            (Some(open), _) if ch == open => quote = None,
            (None, '#') if previous.is_whitespace() => return &raw[..index],
            _ => {}
        }
        previous = ch;
    }
    raw
}

/// Делит содержимое `[a, "b, c", d]` по запятым вне кавычек.
fn split_inline(inner: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in inner.chars() {
        match (quote, ch) {
            (None, '"' | '\'') => {
                quote = Some(ch);
                current.push(ch);
            }
            (Some(open), _) if ch == open => {
                quote = None;
                current.push(ch);
            }
            (None, ',') => parts.push(std::mem::take(&mut current)),
            (None, '[' | ']' | '{' | '}') => return None,
            _ => current.push(ch),
        }
    }
    if quote.is_some() {
        return None;
    }
    parts.push(current);
    Some(parts)
}

/// Запись свойств 020: `#metadata((ключ: значение, …)) <properties>`. `None` —
/// свойств нет или значение записать нельзя.
pub fn properties_record(entries: &[Entry]) -> Option<String> {
    if entries.is_empty() {
        return None;
    }
    let mut pairs = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        pairs.push(format!("{}: {}", render_key(key), render_value(value)?));
    }
    Some(format!(
        "#metadata(({})) <{}>",
        pairs.join(", "),
        crate::properties::RECORD_LABEL
    ))
}

/// Шапка комментарием Typst, читаемым блоком в начале заметки. Закрывающие и
/// вложенные ограничители комментария в тексте разведены пробелом.
pub fn properties_comment(yaml: &str) -> String {
    let safe = yaml.replace("*/", "* /").replace("/*", "/ *");
    format!("/* properties:\n{}\n*/", safe.trim_end())
}

/// Цель вики-ссылки: путь, раздел, блок.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiTarget {
    /// Путь или имя заметки; пусто — ссылка внутри той же заметки.
    pub path: String,
    pub heading: Option<String>,
    pub block: Option<String>,
}

/// Разбирает `Имя`, `Имя#Раздел`, `Имя#^блок`, `#Раздел`, `Имя^блок`.
pub fn parse_wiki_target(raw: &str) -> WikiTarget {
    let raw = raw.trim();
    match raw.split_once('#') {
        Some((path, anchor)) => {
            let anchor = anchor.trim();
            match anchor.strip_prefix('^') {
                Some(block) => WikiTarget {
                    path: path.trim().to_owned(),
                    heading: None,
                    block: Some(block.to_owned()),
                },
                None => WikiTarget {
                    path: path.trim().to_owned(),
                    heading: Some(anchor.to_owned()),
                    block: None,
                },
            }
        }
        None => match raw.split_once('^') {
            Some((path, block)) if !path.is_empty() && !block.is_empty() => WikiTarget {
                path: path.trim().to_owned(),
                heading: None,
                block: Some(block.to_owned()),
            },
            _ => WikiTarget {
                path: raw.to_owned(),
                heading: None,
                block: None,
            },
        },
    }
}

/// Подпись и размер картинки: `300`, `300x200` или `подпись|300`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageHint {
    pub alt: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub fn parse_image_hint(text: &str) -> ImageHint {
    let text = text.trim();
    let (alt, size) = match text.rsplit_once('|') {
        Some((alt, size)) => (alt.trim(), size.trim()),
        None => ("", text),
    };
    if let Some((width, height)) = parse_size(size) {
        return ImageHint {
            alt: alt.to_owned(),
            width: Some(width),
            height,
        };
    }
    ImageHint {
        alt: text.to_owned(),
        ..ImageHint::default()
    }
}

fn parse_size(text: &str) -> Option<(u32, Option<u32>)> {
    let (width, height) = match text.split_once('x') {
        Some((width, height)) => (width, Some(height)),
        None => (text, None),
    };
    let width: u32 = width.parse().ok()?;
    let height = match height {
        Some(height) => Some(height.parse().ok()?),
        None => None,
    };
    (width > 0).then_some((width, height))
}

/// Заголовок выноски: `[!warning]- Заголовок` → (`warning`, `Заголовок`).
pub fn parse_callout(line: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix("[!")?;
    let (kind, after) = rest.split_once(']')?;
    if kind.is_empty() || kind.contains(char::is_whitespace) {
        return None;
    }
    let title = after.trim_start_matches(['+', '-']).trim().to_owned();
    Some((kind.to_lowercase(), title))
}

/// Название выноски без заголовка: `warning` → `Warning`.
pub fn callout_default_title(kind: &str) -> String {
    let mut chars = kind.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_is_split_off_with_its_line_count() {
        let split = split_frontmatter("---\ntitle: a\n---\n# Привет\n");
        assert_eq!(split.yaml, Some("title: a\n"));
        assert_eq!(split.body, "# Привет\n");
        assert_eq!(split.lines, 3);
        assert_eq!(split_frontmatter("текст\n---\nx\n---\n").yaml, None);
        assert_eq!(split_frontmatter("---\nне закрыта\n").yaml, None);
    }

    #[test]
    fn yaml_kinds_map_to_property_values() {
        let yaml = "title: Заметка\ncount: 3\nratio: 0.5\ndone: true\ndate: 2024-05-01\n\
                    tags: [проект, \"два слова\"]\naliases:\n  - один\n  - два\nquoted: \"a: b\"\n\
                    note: |\n  первая\n  вторая\n";
        let entries = parse_yaml(yaml).unwrap();
        let get = |key: &str| &entries.iter().find(|(k, _)| k == key).unwrap().1;
        assert_eq!(get("title"), &PropertyValue::text("Заметка"));
        assert_eq!(get("count"), &PropertyValue::Number { value: 3.0 });
        assert_eq!(get("ratio"), &PropertyValue::Number { value: 0.5 });
        assert_eq!(get("done"), &PropertyValue::Flag { value: true });
        assert_eq!(get("date"), &PropertyValue::date(2024, 5, 1));
        assert_eq!(get("tags"), &PropertyValue::list(["проект", "два слова"]));
        assert_eq!(get("aliases"), &PropertyValue::list(["один", "два"]));
        assert_eq!(get("quoted"), &PropertyValue::text("a: b"));
        assert_eq!(get("note"), &PropertyValue::text("первая\nвторая"));
    }

    #[test]
    fn tags_lose_their_hash_and_a_scalar_becomes_a_list() {
        let entries = parse_yaml("tags: один, два\n").unwrap();
        assert_eq!(entries[0].1, PropertyValue::list(["один", "два"]));
        let entries = parse_yaml("tags:\n  - '#a'\n").unwrap();
        assert_eq!(entries[0].1, PropertyValue::list(["a"]));
    }

    #[test]
    fn nested_yaml_is_not_translated() {
        assert!(parse_yaml("a:\n  b: 1\n").is_none());
        assert!(parse_yaml("a: {b: 1}\n").is_none());
        assert!(parse_yaml("a: &x 1\n").is_none());
    }

    #[test]
    fn the_record_is_read_back_by_the_properties_module() {
        let entries =
            parse_yaml("title: Привет \"друг\"\ntags: [a, b]\nn: 2\nd: 2024-01-02\nok: false\n")
                .unwrap();
        let record = properties_record(&entries).unwrap();
        let read = crate::properties::read(&format!("{record}\n\nТекст"));
        assert!(read.problem.is_none(), "{record}");
        let keys: Vec<&str> = read.properties.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(keys, ["title", "tags", "n", "d", "ok"]);
        assert_eq!(
            read.properties[0].value,
            PropertyValue::text("Привет \"друг\"")
        );
    }

    #[test]
    fn the_comment_cannot_be_closed_early() {
        let comment = properties_comment("a: */ b /* c\n");
        assert_eq!(comment.matches("*/").count(), 1);
        assert_eq!(comment.matches("/*").count(), 1);
    }

    #[test]
    fn wiki_targets_split_into_path_heading_and_block() {
        let t = parse_wiki_target("Заметка#Раздел");
        assert_eq!(
            (t.path.as_str(), t.heading.as_deref()),
            ("Заметка", Some("Раздел"))
        );
        let t = parse_wiki_target("Заметка#^abc123");
        assert_eq!(
            (t.path.as_str(), t.block.as_deref()),
            ("Заметка", Some("abc123"))
        );
        let t = parse_wiki_target("#Раздел");
        assert_eq!(
            (t.path.as_str(), t.heading.as_deref()),
            ("", Some("Раздел"))
        );
        let t = parse_wiki_target("Папка/Имя");
        assert_eq!(t.path, "Папка/Имя");
        assert!(t.heading.is_none() && t.block.is_none());
    }

    #[test]
    fn image_hints_carry_a_size_and_an_alt_text() {
        assert_eq!(parse_image_hint("300").width, Some(300));
        let hint = parse_image_hint("300x200");
        assert_eq!((hint.width, hint.height), (Some(300), Some(200)));
        let hint = parse_image_hint("схема|450");
        assert_eq!((hint.alt.as_str(), hint.width), ("схема", Some(450)));
        let hint = parse_image_hint("просто подпись");
        assert_eq!((hint.alt.as_str(), hint.width), ("просто подпись", None));
    }

    #[test]
    fn callout_titles_are_read() {
        assert_eq!(
            parse_callout("[!warning]- Осторожно"),
            Some(("warning".into(), "Осторожно".into()))
        );
        assert_eq!(
            parse_callout("[!NOTE]"),
            Some(("note".into(), String::new()))
        );
        assert_eq!(parse_callout("обычный текст"), None);
        assert_eq!(callout_default_title("tip"), "Tip");
    }
}
