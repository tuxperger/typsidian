//! Читаемый текст разметки без оформления (research.md §6).
//!
//! Нужен заголовкам и подписям рисунков: в оглавлении стоит то, что прочтёт
//! человек, а не исходная запись. Формула остаётся исходной записью — без неё
//! «$L^2$-нормы» превратилось бы в «-нормы».

use typst::syntax::{ast, LinkedNode, SyntaxKind};

/// Самый длинный кусок исходной записи, который показывается вместо текста.
pub const SOURCE_FALLBACK_CHARS: usize = 60;

/// Читаемый текст узла разметки (`Markup`, блока содержимого, заголовка).
/// Пробелы схлопнуты, края обрезаны; пусто — если читать нечего.
pub fn plain_text(node: &LinkedNode) -> String {
    let mut out = String::new();
    collect(node, &mut out);
    collapse(&out)
}

/// Начало исходной записи для запасного показа: без ведущих `#` и `=`, пробелы
/// схлопнуты, не длиннее [`SOURCE_FALLBACK_CHARS`] знаков.
pub fn source_head(node: &LinkedNode) -> String {
    let source = node.full_text();
    let stripped = source.trim_start_matches(|c: char| c == '#' || c == '=' || c.is_whitespace());
    let text = collapse(if collapse(stripped).is_empty() {
        &source
    } else {
        stripped
    });
    text.chars().take(SOURCE_FALLBACK_CHARS).collect()
}

/// Схлопывает любые пробельные цепочки в один пробел и обрезает края.
pub(crate) fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect(node: &LinkedNode, out: &mut String) {
    match node.kind() {
        SyntaxKind::Text => out.push_str(node.leaf_text()),
        SyntaxKind::Space | SyntaxKind::Linebreak | SyntaxKind::Parbreak => out.push(' '),
        SyntaxKind::Escape => {
            if let Some(escape) = node.cast::<ast::Escape>() {
                out.push(escape.get());
            }
        }
        SyntaxKind::Shorthand => {
            if let Some(shorthand) = node.cast::<ast::Shorthand>() {
                out.push(shorthand.get());
            }
        }
        SyntaxKind::SmartQuote => {
            let double = node
                .cast::<ast::SmartQuote>()
                .is_some_and(|quote| quote.double());
            out.push(if double { '"' } else { '\'' });
        }
        SyntaxKind::Raw => {
            if let Some(raw) = node.cast::<ast::Raw>() {
                let lines: Vec<&str> = raw.lines().map(|line| line.get().as_str()).collect();
                out.push_str(&lines.join(" "));
            }
        }
        SyntaxKind::Link => out.push_str(node.leaf_text()),
        SyntaxKind::Ref => {
            if let Some(reference) = node.cast::<ast::Ref>() {
                out.push('@');
                out.push_str(reference.target());
            }
        }
        SyntaxKind::Equation => {
            for child in node.children() {
                if child.kind() == SyntaxKind::Math {
                    out.push_str(&child.full_text());
                }
            }
        }
        // Вызов функции: только блоки содержимого (`#emph[важно]` → «важно»),
        // остальные аргументы — настройки, а не текст.
        SyntaxKind::FuncCall => {
            for child in node.children() {
                if child.kind() == SyntaxKind::Args {
                    for arg in child.children() {
                        if arg.kind() == SyntaxKind::ContentBlock {
                            collect(&arg, out);
                        }
                    }
                }
            }
        }
        SyntaxKind::Hash
        | SyntaxKind::Label
        | SyntaxKind::LineComment
        | SyntaxKind::BlockComment
        | SyntaxKind::Error => {}
        // Markup, ContentBlock, Strong, Emph и прочие составные узлы: их
        // знаки-обрамления (`*`, `_`, `[`) листья, не названные выше, и
        // пропускаются сами.
        _ => {
            for child in node.children() {
                collect(&child, out);
            }
        }
    }
}

/// Строковое значение литерала: `"Подпись"` → `Подпись`.
pub(crate) fn string_literal(node: &LinkedNode) -> Option<String> {
    node.cast::<ast::Str>().map(|text| text.get().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use typst::syntax::parse;

    fn heading_text(source: &str) -> String {
        let root = parse(source);
        let linked = LinkedNode::new(&root);
        let heading = linked
            .children()
            .find(|child| child.kind() == SyntaxKind::Heading)
            .expect("заголовок");
        plain_text(&heading)
    }

    #[test]
    fn markup_is_reduced_to_words() {
        assert_eq!(heading_text("= Привет, *мир* и _все_"), "Привет, мир и все");
        assert_eq!(heading_text("= a\\#b  c"), "a#b c");
        assert_eq!(
            heading_text("= `код` и https://x.org"),
            "код и https://x.org"
        );
        assert_eq!(heading_text("= см. @intro"), "см. @intro");
        assert_eq!(heading_text("= #emph[важно] и #text(red)[A]"), "важно и A");
        assert_eq!(heading_text("= О $L^2$-нормах"), "О L^2-нормах");
    }

    #[test]
    fn nothing_readable_gives_an_empty_text() {
        assert_eq!(heading_text("= #image(\"a.png\")"), "");
        assert_eq!(heading_text("= <only-label>"), "");
    }
}
