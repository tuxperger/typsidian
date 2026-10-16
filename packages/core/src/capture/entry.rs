//! Запись захвата в тексте заметки и дописывание в конец (research.md §2, §3).
//!
//! Чистые функции: ни vault, ни часов. Поэтому вся таблица «вид захвата ×
//! вид записи × время» проверяется одинаково под `cargo test` и
//! `wasm-bindgen-test` (принцип II).

use super::{Capture, CaptureContent, CaptureSettings, EntryStyle};
use crate::elements::edit::TextEdit;
use crate::import::markdown::{escape_markup, string_literal};

/// Разделитель содержимого и приписки.
const NOTE_SEPARATOR: &str = " — ";

/// Отступ продолжения пункта списка: строки текста остаются внутри пункта.
const LIST_INDENT: &str = "  ";

/// Запись захвата разметкой Typst, без завершающего перевода строки.
///
/// `image_path` — путь вставки картинки относительно заметки назначения (его
/// знает только запись вложения, [`super::write`]); без него — имя файла, чего
/// хватает для образца записи в настройках.
///
/// Текст пользователя экранируется (те же правила, что в импорте 021): ничто
/// из захваченного не превращается в заголовок, список или код.
pub fn entry(settings: &CaptureSettings, capture: &Capture, image_path: Option<&str>) -> String {
    let marker = match settings.entry_style {
        EntryStyle::List => "- ",
        EntryStyle::Paragraph => "",
    };
    let time = if settings.entry_time {
        format!("{:02}:{:02} ", capture.at.hour, capture.at.minute)
    } else {
        String::new()
    };
    let lead = format!("{marker}{time}");

    let body = match &capture.content {
        CaptureContent::Text { text } => text_markup(text, settings.entry_style, lead.is_empty()),
        CaptureContent::Link { url, title } => {
            let title = title
                .as_deref()
                .map(str::trim)
                .filter(|title| !title.is_empty() && *title != url.trim());
            match title {
                Some(title) => format!(
                    "#link({})[{}]",
                    string_literal(url.trim()),
                    escape_markup(title, false)
                ),
                None => format!("#link({})", string_literal(url.trim())),
            }
        }
        CaptureContent::Image { name, .. } => {
            format!("#image({})", string_literal(image_path.unwrap_or(name)))
        }
    };

    let note = capture
        .note
        .as_deref()
        .map(str::trim)
        .filter(|note| !note.is_empty())
        .map(|note| escape_markup(note, false));
    let tail = match (body.is_empty(), note) {
        (_, None) => body,
        (true, Some(note)) => note,
        (false, Some(note)) => format!("{body}{NOTE_SEPARATOR}{note}"),
    };
    format!("{lead}{tail}")
}

/// Многострочный текст как разметка: каждая строка экранирована отдельно.
///
/// В списке строки продолжения получают отступ, чтобы остаться в том же
/// пункте. Пустые строки по краям отбрасываются, пробелы в конце строк — тоже.
fn text_markup(text: &str, style: EntryStyle, first_line_starts: bool) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.trim().lines().map(str::trim_end).collect();
    let mut out = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            out.push('\n');
            if style == EntryStyle::List && !line.is_empty() {
                out.push_str(LIST_INDENT);
            }
        }
        out.push_str(&escape_markup(line, index > 0 || first_line_starts));
    }
    out
}

/// Результат дописывания: новый текст и та же вставка в единицах UTF-16.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appended {
    pub new_text: String,
    pub edit: TextEdit,
}

/// Дописывает запись в конец текста (FR-003).
///
/// Прежний текст не меняется ни на байт: результат — он же плюс вставка в
/// самом конце. Вставка начинается с перевода строки, если прежний текст не
/// кончался им; для абзаца перед записью оставляется пустая строка. Запись
/// всегда завершается переводом строки, поэтому следующий захват встаёт
/// ровно под ней.
pub fn append(text: &str, style: EntryStyle, entry: &str) -> Appended {
    let separator = if text.is_empty() {
        ""
    } else {
        match style {
            EntryStyle::List if text.ends_with('\n') => "",
            EntryStyle::List => "\n",
            EntryStyle::Paragraph if text.ends_with("\n\n") => "",
            EntryStyle::Paragraph if text.ends_with('\n') => "\n",
            EntryStyle::Paragraph => "\n\n",
        }
    };
    let insert = format!("{separator}{entry}\n");
    let at = text.encode_utf16().count();
    let mut new_text = String::with_capacity(text.len() + insert.len());
    new_text.push_str(text);
    new_text.push_str(&insert);
    Appended {
        new_text,
        edit: TextEdit {
            from: at,
            to: at,
            insert,
        },
    }
}
