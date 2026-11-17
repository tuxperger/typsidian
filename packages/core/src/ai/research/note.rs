//! Заметка Typst с результатом исследования (фича 056, research.md §4). Текст собирает ядро,
//! а не модель: всё, что написала модель (вопрос, абзацы, названия источников), экранируется,
//! чтобы ни `#`, ни `$`, ни `<метка>` в нём не стали разметкой или кодом; ссылки идут только
//! через `#link("…")` со строкой в кавычках и только по проверенным адресам.

use super::{ResearchKind, ResearchSource};
use crate::ai::lesson::file::Language;

/// Экранирует текст модели для разметки Typst. Построчно: управляющие символы и символы
/// форматирования (направление письма, невидимые разделители) выбрасываются, любой пробельный
/// символ становится пробелом, спецсимволы получают обратную косую черту. Знаки списка,
/// заголовка и нумерации экранируются в начале строки. Обратная косая черта перед любым
/// непробельным символом в Typst — просто он сам, поэтому экранирование безопасно.
pub fn escape_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    for (index, line) in text.lines().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        escape_line(line, &mut out);
    }
    out
}

fn escape_line(line: &str, out: &mut String) {
    // Пока в строке были только пробелы — «начало»; пока только цифры — «номер».
    let mut at_start = true;
    let mut digits = 0usize;
    let mut only_digits = true;
    for ch in line.chars() {
        if ch.is_whitespace() {
            out.push(' ');
            continue;
        }
        if ch.is_control() || crate::ai::lesson::is_format_char(ch) {
            continue;
        }
        let special = matches!(
            ch,
            '\\' | '#' | '$' | '*' | '_' | '`' | '<' | '>' | '@' | '[' | ']' | '~' | '/'
        );
        let marker = at_start && matches!(ch, '-' | '+' | '=');
        let numbered = ch == '.' && only_digits && digits > 0;
        if special || marker || numbered {
            out.push('\\');
        }
        out.push(ch);
        if ch.is_ascii_digit() && only_digits {
            digits += 1;
        } else {
            only_digits = false;
        }
        at_start = false;
    }
}

/// Одна строка: переводы строк заменяются пробелами.
pub fn escape_inline(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    escape_markup(flat.trim())
}

/// Строка Typst в кавычках. Адреса уже проверены (`clean_url`), но экранирование стоит и здесь.
fn string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        if ch.is_control() {
            continue;
        }
        if matches!(ch, '"' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

/// Всё, что нужно для сборки заметки.
pub struct NoteInput<'a> {
    pub language: Language,
    pub kind: ResearchKind,
    pub question: &'a str,
    /// Путь PDF для заметки по документу.
    pub pdf: Option<&'a str>,
    pub model: &'a str,
    pub paragraphs: &'a [String],
    pub sources: &'a [ResearchSource],
    /// Прочитана не вся книга: часть страниц не вошла в запрос.
    pub truncated: bool,
}

fn pages_text(language: Language, pages: &[u32]) -> String {
    let label = match language {
        Language::Ru => "стр.",
        Language::En => "p.",
    };
    let list: Vec<String> = pages.iter().map(u32::to_string).collect();
    format!("{label} {}", list.join(", "))
}

/// Собирает заметку Typst: вопрос, пометка «найдено моделью», абзацы, источники.
pub fn build_note(input: &NoteInput<'_>) -> String {
    let (marker_web, marker_pdf, sources_heading, none, truncated, by, check) = match input.language
    {
        Language::Ru => (
            "Найдено моделью с помощью веб-поиска провайдера",
            "Составлено моделью по тексту PDF",
            "Источники",
            "Источников нет: модель их не назвала.",
            "Прочитана только начальная часть документа.",
            "Модель",
            "Приложение источники не проверяет: проверьте их сами.",
        ),
        Language::En => (
            "Found by the model using the provider's web search",
            "Written by the model from the text of a PDF",
            "Sources",
            "No sources: the model named none.",
            "Only the beginning of the document was read.",
            "Model",
            "The app does not check the sources: check them yourself.",
        ),
    };
    let mut out = String::new();
    out.push_str(&format!("= {}\n\n", escape_inline(input.question)));
    let marker = match input.kind {
        ResearchKind::Web => marker_web,
        ResearchKind::Pdf => marker_pdf,
    };
    out.push_str(&format!(
        "_{marker}. {by}: {}. {check}_\n\n",
        escape_inline(input.model)
    ));
    if let Some(pdf) = input.pdf {
        out.push_str(&format!("PDF: {}\n\n", escape_inline(pdf)));
    }
    if input.truncated {
        out.push_str(&format!("_{truncated}_\n\n"));
    }
    for paragraph in input.paragraphs {
        out.push_str(&escape_markup(paragraph));
        out.push_str("\n\n");
    }
    out.push_str(&format!("== {sources_heading}\n\n"));
    if input.sources.is_empty() {
        out.push_str(none);
        out.push('\n');
    }
    for source in input.sources {
        out.push_str("- ");
        out.push_str(&escape_inline(&source.title));
        if let Some(url) = &source.url {
            out.push_str(&format!(" — #link({})", string_literal(url)));
        }
        if let Some(pages) = &source.pages {
            out.push_str(&format!(", {}", pages_text(input.language, pages)));
        }
        out.push('\n');
    }
    out
}
