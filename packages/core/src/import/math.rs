//! Формулы LaTeX → Typst (фича 021, FR-007, research.md §5).
//!
//! Перевод делает `mitex`, но его вывод рассчитан на пакет Typst
//! `@preview/mitex` (вспомогательные `mitexsqrt`, `pmatrix`, `textmath` и
//! другие) — без сети и без пакета в заметке он не соберётся. Поэтому вывод
//! проходит три шага: замена вспомогательных имён на встроенные средства Typst,
//! проверка синтаксиса и проверка, что каждое имя длиннее одной буквы есть в
//! стандартной библиотеке Typst. Не прошло хоть что-то — `None`: вызывающий
//! оставит исходный текст формулы видимым и внесёт потерю в отчёт.
//!
//! Полная пробная сборка формулы (`PreviewWorld`) не делается намеренно: каждая
//! сборка разбирает встроенные шрифты заново, на заметках с сотнями формул это
//! секунды. Заменяет её проверка имён по библиотеке; реальную сборку всех
//! образцов выполняют тесты (SC-001).

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;

use typst::{Library, LibraryExt};

/// Вид формулы в тексте.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathMode {
    /// `$…$` — внутри строки.
    Inline,
    /// `$$…$$` — отдельной строкой.
    Display,
}

/// Формула LaTeX как запись Typst вместе с `$`. `None` — перевести надёжно не
/// удалось.
pub fn convert(latex: &str, mode: MathMode) -> Option<String> {
    let latex = latex.trim();
    if latex.is_empty() {
        return None;
    }
    // `mitex` на странном вводе не паникует, но чужой разбор не должен ронять
    // импорт: под wasm32 с `panic=abort` перехват не сработает, под native —
    // спасёт заметку.
    let converted = catch_unwind(AssertUnwindSafe(|| mitex::convert_math(latex, None)))
        .ok()?
        .ok()?;
    let typst = rewrite(&converted)?;
    let typst = typst.trim();
    if typst.is_empty() || !is_safe(typst) {
        return None;
    }
    Some(match mode {
        MathMode::Inline => format!("${typst}$"),
        MathMode::Display => format!("$ {typst} $"),
    })
}

/// Формула не может закрыть `$` раньше времени и не содержит комментариев,
/// которые съели бы закрывающий `$`; разбирается как математика Typst; все
/// имена известны библиотеке.
fn is_safe(typst: &str) -> bool {
    let mut previous = ' ';
    for ch in typst.chars() {
        if ch == '$' && previous != '\\' {
            return false;
        }
        previous = ch;
    }
    if typst.contains("//") || typst.contains("/*") || typst.contains("*/") {
        return false;
    }
    if typst::syntax::parse_math(typst).diagnosis().errors {
        return false;
    }
    names_are_known(typst)
}

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(<Library as LibraryExt>::default)
}

fn is_known(name: &str) -> bool {
    let library = library();
    library.math.scope().get(name).is_some() || library.global.scope().get(name).is_some()
}

/// Каждое имя длиннее одной буквы (кроме модификаторов после точки, именованных
/// аргументов и содержимого строк) обязано существовать в библиотеке.
fn names_are_known(typst: &str) -> bool {
    let chars: Vec<char> = typst.chars().collect();
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let ch = chars[i];
        if in_string {
            if ch == '\\' {
                i += 1;
            } else if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if ch == '"' {
            in_string = true;
            i += 1;
            continue;
        }
        if ch == '\\' {
            // Экранированный знак: `\(`, `\{`, `\\`.
            i += 2;
            continue;
        }
        if ch.is_alphabetic() {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric()) {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            // Сегменты после точки — модификаторы символа: `dot.c`.
            let mut tail = i;
            while tail + 1 < chars.len() && chars[tail] == '.' && chars[tail + 1].is_alphabetic() {
                tail += 1;
                while tail < chars.len() && chars[tail].is_alphanumeric() {
                    tail += 1;
                }
            }
            let named_argument = chars.get(tail) == Some(&':');
            i = tail;
            if name.chars().count() > 1 && !named_argument && !is_known(&name) {
                return false;
            }
            continue;
        }
        i += 1;
    }
    true
}

// ── Замена вспомогательных имён mitex ───────────────────────────────────────

/// Матрицы `mitex` → `mat(delim: …)`.
fn matrix_delim(name: &str) -> Option<&'static str> {
    Some(match name {
        "pmatrix" => "\"(\"",
        "bmatrix" => "\"[\"",
        "Bmatrix" => "\"{\"",
        "vmatrix" => "\"|\"",
        "Vmatrix" => "\"||\"",
        "matrix" => "#none",
        _ => return None,
    })
}

/// Подставляет вместо вспомогательных вызовов `mitex` встроенные средства
/// Typst. `None` — встретилось вспомогательное, для которого замены нет.
fn rewrite(source: &str) -> Option<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' {
            out.push(ch);
            if let Some(next) = chars.get(i + 1) {
                out.push(*next);
            }
            i += 2;
            continue;
        }
        if ch == '#' {
            let (text, next) = rewrite_text_call(&chars, i)?;
            out.push_str(&text);
            i = next;
            continue;
        }
        if ch.is_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_alphanumeric() {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            if chars.get(i) == Some(&'(') && is_helper(&name) {
                let close = matching_paren(&chars, i)?;
                let inner: String = chars[i + 1..close].iter().collect();
                out.push_str(&rewrite_helper(&name, &inner)?);
                i = close + 1;
                continue;
            }
            out.push_str(&name);
            continue;
        }
        out.push(ch);
        i += 1;
    }
    Some(out)
}

fn is_helper(name: &str) -> bool {
    matrix_delim(name).is_some()
        || matches!(
            name,
            "mitexsqrt"
                | "mitexunderbrace"
                | "mitexoverbrace"
                | "mitexmathbf"
                | "mitexdisplay"
                | "mitexlabel"
                | "dfrac"
                | "tfrac"
                | "pmod"
                | "operatorname"
                | "phantom"
        )
}

fn rewrite_helper(name: &str, inner: &str) -> Option<String> {
    if let Some(delim) = matrix_delim(name) {
        let body = rewrite(&inner.replace("zws", ""))?;
        return Some(if delim == "#none" {
            format!("mat({body})")
        } else {
            format!("mat(delim: {delim}, {body})")
        });
    }
    let args = split_args(inner);
    let one = |index: usize| -> Option<String> { rewrite(args.get(index)?.trim()) };
    Some(match name {
        "mitexsqrt" => match args.as_slice() {
            [radicand] => format!("sqrt({})", rewrite(radicand.trim())?),
            [root, radicand] => {
                let root = root.trim();
                let root = root.strip_prefix("\\[")?.strip_suffix("\\]")?;
                format!(
                    "root({}, {})",
                    rewrite(root.trim())?,
                    rewrite(radicand.trim())?
                )
            }
            _ => return None,
        },
        "mitexunderbrace" => format!("underbrace({})", one(0)?),
        "mitexoverbrace" => format!("overbrace({})", one(0)?),
        "mitexmathbf" => format!("bold(upright({}))", one(0)?),
        "mitexdisplay" => format!("display({})", rewrite(inner.trim())?),
        "mitexlabel" => String::new(),
        "dfrac" => format!("display(frac({}, {}))", one(0)?, one(1)?),
        "tfrac" => format!("inline(frac({}, {}))", one(0)?, one(1)?),
        "pmod" => format!("(mod {})", one(0)?),
        "phantom" => format!("hide({})", one(0)?),
        // `operatorname(f o o )` — буквы через пробел; в Typst это `op("foo")`.
        "operatorname" => {
            let word: String = inner.chars().filter(|ch| !ch.is_whitespace()).collect();
            if word.is_empty() || word.chars().any(|ch| !ch.is_alphanumeric()) {
                return None;
            }
            format!("op(\"{word}\")")
        }
        _ => return None,
    })
}

/// `#textmath[текст];` и родственные: текст в формуле становится строкой.
/// Возвращает запись Typst и позицию после вызова.
fn rewrite_text_call(chars: &[char], at: usize) -> Option<(String, usize)> {
    let mut i = at + 1;
    let start = i;
    while i < chars.len() && chars[i].is_alphanumeric() {
        i += 1;
    }
    let name: String = chars[start..i].iter().collect();
    if chars.get(i) != Some(&'[') {
        return None;
    }
    let mut depth = 0usize;
    let mut j = i;
    let close = loop {
        match chars.get(j)? {
            '\\' => j += 1,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break j;
                }
            }
            _ => {}
        }
        j += 1;
    };
    let body: String = chars[i + 1..close].iter().collect();
    // Знак `\x` внутри текста — просто сам знак.
    let mut text = String::new();
    let mut escaped = false;
    for ch in body.chars() {
        if escaped {
            text.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            text.push(ch);
        }
    }
    let literal = format!(
        "\"{}\"",
        text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', " ")
    );
    let value = match name.as_str() {
        "textmath" | "text" | "textrm" | "textnormal" | "mbox" => literal,
        "textbf" => format!("bold({literal})"),
        "textit" | "emph" => format!("italic({literal})"),
        _ => return None,
    };
    let mut next = close + 1;
    if chars.get(next) == Some(&';') {
        next += 1;
    }
    Some((value, next))
}

/// Закрывающая скобка для `(` на позиции `open`; `\(` и `\)` — знаки, а не
/// скобки вызова.
fn matching_paren(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    let mut in_string = false;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            '"' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Делит аргументы по запятым верхнего уровня.
fn split_args(inner: &str) -> Vec<String> {
    let chars: Vec<char> = inner.chars().collect();
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '\\' => {
                current.push(ch);
                if let Some(next) = chars.get(i + 1) {
                    current.push(*next);
                }
                i += 2;
                continue;
            }
            '"' => in_string = !in_string,
            '(' | '[' if !in_string => depth += 1,
            ')' | ']' if !in_string => depth -= 1,
            ',' if !in_string && depth == 0 => {
                args.push(std::mem::take(&mut current));
                i += 1;
                continue;
            }
            _ => {}
        }
        current.push(ch);
        i += 1;
    }
    args.push(current);
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inline(latex: &str) -> Option<String> {
        convert(latex, MathMode::Inline)
    }

    #[test]
    fn simple_formulas_become_typst_math() {
        assert_eq!(inline(r"\frac{a}{b}").as_deref(), Some("$frac(a ,b )$"));
        assert_eq!(
            inline(r"\sum_{i=0}^{n} i").as_deref(),
            Some("$sum _(i = 0 )^(n ) i$")
        );
        assert_eq!(
            inline(r"x_1^2 + \alpha").as_deref(),
            Some("$x _(1 )^(2 ) +  alpha$")
        );
    }

    #[test]
    fn display_math_has_spaces_after_the_dollar() {
        let typst = convert(r"\int_0^1 f(x)\,dx", MathMode::Display).unwrap();
        assert!(typst.starts_with("$ ") && typst.ends_with(" $"), "{typst}");
    }

    #[test]
    fn mitex_helpers_are_replaced_by_builtins() {
        assert_eq!(inline(r"\sqrt{2}").as_deref(), Some("$sqrt(2)$"));
        assert_eq!(inline(r"\sqrt[3]{x}").as_deref(), Some("$root(3, x)$"));
        assert_eq!(
            inline(r"\text{hello world}").as_deref(),
            Some("$\"hello world\"$")
        );
        let matrix = inline(r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}").unwrap();
        assert!(matrix.starts_with("$mat(delim: \"(\","), "{matrix}");
        assert!(!matrix.contains("zws"));
        assert_eq!(
            inline(r"\operatorname{foo}(x)").as_deref(),
            Some("$op(\"foo\")\\(x \\)$")
        );
        assert!(inline(r"\mathbf{x}").unwrap().contains("bold(upright(x"));
    }

    #[test]
    fn what_cannot_be_translated_is_none() {
        // Неизвестная команда.
        assert_eq!(inline(r"\unknowncmd{x}"), None);
        // Помощник пакета без замены.
        assert_eq!(inline(r"\substack{a \\ b}"), None);
        assert_eq!(inline(r"\hspace{1em}"), None);
        // Пустая формула.
        assert_eq!(inline("   "), None);
        assert_eq!(inline(r"\label{eq}"), None);
    }

    #[test]
    fn every_translated_formula_parses_as_typst_math() {
        for latex in [
            r"\alpha + \beta = \gamma",
            r"\left( \frac{a}{b} \right)",
            r"\lim_{x \to 0} \frac{\sin x}{x} = 1",
            r"\begin{cases} 1 & x>0 \\ 0 & \text{else} \end{cases}",
            r"\begin{bmatrix} 1 & 2 \\ 3 & 4 \end{bmatrix}",
            r"\mathbb{R}^n \subset \mathcal{L}",
            r"\vec{v} \cdot \hat{n} \leq \infty",
        ] {
            let typst = convert(latex, MathMode::Display).unwrap_or_else(|| panic!("{latex}"));
            assert!(
                !typst::syntax::parse(&typst).diagnosis().errors,
                "{latex} -> {typst}"
            );
        }
    }
}
