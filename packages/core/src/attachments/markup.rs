//! Текст вставки и правка заметки (FR-001, FR-005, research.md §9).
//!
//! Правило переносов — одно для трёх оболочек и трёх источников. Правка
//! считается поверх `templates::plan_insertion`: замена выделения и место
//! каретки — те же, что у вставки заготовки (003), и разойтись они не могут.

use crate::templates::{plan_insertion, InsertionEdit, InsertionPoint};

/// Запись изображения в тексте заметки (обязательство 7).
pub fn image_markup(markup_path: &str) -> String {
    let escaped = markup_path.replace('\\', "\\\\").replace('"', "\\\"");
    format!("#image(\"{escaped}\")")
}

/// Правка заметки для вставленных файлов (обязательство 8).
///
/// `before`/`after` — текст строки до и после точки вставки. `block = false` —
/// записи встают в точку как есть, каретка за ними; `block = true` — по записи
/// на строку, ведущий и завершающий перенос только там, где в строке есть
/// непробельный текст. Пустой список путей — `None`: текст не меняется.
pub fn plan_markup(
    text_length: usize,
    point: &InsertionPoint,
    before: &str,
    after: &str,
    markup_paths: &[String],
    block: bool,
) -> Option<InsertionEdit> {
    if markup_paths.is_empty() {
        return None;
    }
    let lines: Vec<String> = markup_paths.iter().map(|path| image_markup(path)).collect();
    let mut text = lines.join("\n");
    if block {
        if before.chars().any(|character| !character.is_whitespace()) {
            text.insert(0, '\n');
        }
        if after.chars().any(|character| !character.is_whitespace()) {
            text.push('\n');
        }
    }
    Some(plan_insertion(text_length, point, &text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::TextSpan;

    fn at(offset: usize) -> InsertionPoint {
        InsertionPoint {
            offset,
            selection: None,
        }
    }

    #[test]
    fn escapes_backslash_and_quote() {
        assert_eq!(image_markup(r#"a"b\c.png"#), r#"#image("a\"b\\c.png")"#);
    }

    #[test]
    fn inline_insert_puts_the_caret_after_the_markup() {
        let edit = plan_markup(10, &at(4), "Текст", " дальше", &["x.png".into()], false).unwrap();
        assert_eq!(edit.text, "#image(\"x.png\")");
        assert_eq!(edit.replace, TextSpan { start: 4, end: 4 });
        assert_eq!(edit.caret, 4 + edit.text.encode_utf16().count());
    }

    #[test]
    fn block_insert_breaks_lines_only_around_text() {
        let paths = vec!["a.png".to_string(), "b.png".to_string()];
        let edit = plan_markup(0, &at(0), "  ", "", &paths, true).unwrap();
        assert_eq!(edit.text, "#image(\"a.png\")\n#image(\"b.png\")");
        let edit = plan_markup(20, &at(5), "слово", "ещё", &paths, true).unwrap();
        assert_eq!(edit.text, "\n#image(\"a.png\")\n#image(\"b.png\")\n");
    }

    #[test]
    fn selection_is_replaced_and_no_paths_means_no_edit() {
        let point = InsertionPoint {
            offset: 2,
            selection: Some(TextSpan { start: 2, end: 6 }),
        };
        let edit = plan_markup(10, &point, "", "", &["a.png".into()], false).unwrap();
        assert_eq!(edit.replace, TextSpan { start: 2, end: 6 });
        assert!(plan_markup(10, &at(0), "", "", &[], false).is_none());
    }
}
