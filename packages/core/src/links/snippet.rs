//! Место вхождения и фрагмент-окно для строки панели (research.md §6).
//!
//! Место считается теми же правилами, что `preview::diagnostics`: строка и
//! колонка с единицы, колонка — в символах (не в байтах и не в единицах UTF-16),
//! перевод строки — любой из тех, что знает Typst. Расхождение с диагностикой
//! сдвинуло бы каретку при переходе, поэтому совпадение проверяется тестом на
//! одном и том же тексте.

use typst::syntax::is_newline;

use super::SitePlace;
use crate::search::LineSegment;

/// Сколько символов строки показывается по каждую сторону от вхождения.
pub const WINDOW_CHARS: usize = 48;

/// Многоточие на месте обрезки.
const ELLIPSIS: &str = "…";

/// Место байтового диапазона `start..end` в `text`.
///
/// Диапазон обязан лежать на границах символов; иначе границы прижимаются
/// влево — паники из-за чужой ошибки на границе быть не должно.
pub fn place_of(text: &str, start: usize, end: usize) -> SitePlace {
    let start = floor_boundary(text, start);
    let end = floor_boundary(text, end).max(start);
    let (line_start, line) = line_start_of(text, start);
    SitePlace {
        line: line as u32 + 1,
        column: text[line_start..start].chars().count() as u32 + 1,
        byte_start: start as u32,
        byte_end: end as u32,
    }
}

/// Ближайшая граница символа не правее `index`.
fn floor_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Начало строки, в которой лежит байт `at`, и её номер с нуля.
///
/// `\r\n` — один перевод строки, как в `typst::syntax::Lines`.
fn line_start_of(text: &str, at: usize) -> (usize, usize) {
    let mut line = 0;
    let mut line_start = 0;
    let mut chars = text[..at].char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if !is_newline(ch) {
            continue;
        }
        let mut next = index + ch.len_utf8();
        if ch == '\r' {
            if let Some(&(after, '\n')) = chars.peek() {
                chars.next();
                next = after + 1;
            } else if text[next..].starts_with('\n') {
                // `\r` в самом конце среза, `\n` сразу за срезом: вхождение
                // начинается посреди пары, строка та же.
                continue;
            }
        }
        line += 1;
        line_start = next;
    }
    (line_start, line)
}

/// Конец строки, начавшейся в `from`: байт перед первым переводом строки.
fn line_end_from(text: &str, from: usize) -> usize {
    text[from..]
        .char_indices()
        .find(|(_, ch)| is_newline(*ch))
        .map(|(index, _)| from + index)
        .unwrap_or(text.len())
}

/// Фрагмент строки вокруг вхождения `start..end`: строка вхождения, обрезанная
/// окном в [`WINDOW_CHARS`] символов по каждую сторону по границе слова, с «…»
/// на месте обрезки. Подсвечено ровно это вхождение; многострочное
/// показывается первой строкой.
pub fn window(text: &str, start: usize, end: usize) -> Vec<LineSegment> {
    let start = floor_boundary(text, start);
    let end = floor_boundary(text, end).max(start);
    let (line_start, _) = line_start_of(text, start);
    let line_end = line_end_from(text, line_start);
    let end = end.min(line_end).max(start);

    let before = &text[line_start..start];
    let after = &text[end..line_end];

    let mut segments = Vec::with_capacity(3);
    let before = cut_left(before);
    if !before.is_empty() {
        segments.push(LineSegment {
            text: before,
            highlighted: false,
        });
    }
    segments.push(LineSegment {
        text: text[start..end].to_owned(),
        highlighted: true,
    });
    let after = cut_right(after);
    if !after.is_empty() {
        segments.push(LineSegment {
            text: after,
            highlighted: false,
        });
    }
    segments
}

/// Хвост текста перед вхождением: не больше окна, начало — по границе слова.
fn cut_left(before: &str) -> String {
    let count = before.chars().count();
    if count <= WINDOW_CHARS {
        return before.trim_start().to_owned();
    }
    let cut = before
        .char_indices()
        .nth(count - WINDOW_CHARS)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let mid_word = before[..cut]
        .chars()
        .next_back()
        .is_some_and(|ch| !ch.is_whitespace())
        && before[cut..]
            .chars()
            .next()
            .is_some_and(|ch| !ch.is_whitespace());
    let mut from = cut;
    if mid_word {
        // Резать посреди слова не годится: уходим к началу следующего слова,
        // а если в окне оно одно, оставляем как есть.
        if let Some((index, ch)) = before[cut..]
            .char_indices()
            .find(|(_, ch)| ch.is_whitespace())
        {
            from = cut + index + ch.len_utf8();
        }
    }
    format!("{ELLIPSIS}{}", before[from..].trim_start())
}

/// Начало текста после вхождения: не больше окна, конец — по границе слова.
fn cut_right(after: &str) -> String {
    let count = after.chars().count();
    if count <= WINDOW_CHARS {
        return after.trim_end().to_owned();
    }
    let cut = after
        .char_indices()
        .nth(WINDOW_CHARS)
        .map(|(index, _)| index)
        .unwrap_or(after.len());
    let mid_word = after[..cut]
        .chars()
        .next_back()
        .is_some_and(|ch| !ch.is_whitespace())
        && after[cut..]
            .chars()
            .next()
            .is_some_and(|ch| !ch.is_whitespace());
    let mut to = cut;
    if mid_word {
        if let Some(index) = after[..cut].rfind(char::is_whitespace) {
            to = index;
        }
    }
    format!("{}{ELLIPSIS}", after[..to].trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use typst::syntax::Source;

    fn plain(segments: &[LineSegment]) -> String {
        segments.iter().map(|s| s.text.as_str()).collect()
    }

    fn highlighted(segments: &[LineSegment]) -> Vec<&str> {
        segments
            .iter()
            .filter(|s| s.highlighted)
            .map(|s| s.text.as_str())
            .collect()
    }

    #[test]
    fn place_counts_columns_in_chars_from_one() {
        // 😀 вне BMP: 4 байта, 2 единицы UTF-16, 1 символ.
        let text = "первая\nа😀б [[цель]]";
        let start = text.find("[[").unwrap();
        let place = place_of(text, start, start + "[[цель]]".len());
        assert_eq!((place.line, place.column), (2, 5));
        assert_eq!(place.byte_start as usize, start);
        assert_eq!(place.byte_end as usize, start + "[[цель]]".len());
    }

    #[test]
    fn place_matches_the_compiler_lines_on_every_occurrence() {
        let text = "а [[x]]\r\nбб😀 [[x]]\n\n\u{2028}ёё[[x]]\r[[x]] кириллица [[x]]";
        let source = Source::detached(text);
        let lines = source.lines();
        for (start, _) in text.match_indices("[[x]]") {
            let (line, column) = lines.byte_to_line_column(start).unwrap();
            let place = place_of(text, start, start + 5);
            assert_eq!(
                (place.line as usize, place.column as usize),
                (line + 1, column + 1),
                "байт {start}"
            );
        }
    }

    #[test]
    fn window_keeps_a_short_line_whole() {
        let text = "см. [[Проект]] завтра";
        let start = text.find("[[").unwrap();
        let segments = window(text, start, start + "[[Проект]]".len());
        assert_eq!(plain(&segments), "см. [[Проект]] завтра");
        assert_eq!(highlighted(&segments), vec!["[[Проект]]"]);
    }

    #[test]
    fn window_at_line_edges_has_no_empty_segments() {
        let text = "[[a]]";
        let segments = window(text, 0, text.len());
        assert_eq!(segments.len(), 1);
        assert!(segments[0].highlighted);

        let text = "слово [[a]]";
        let start = text.find("[[").unwrap();
        let segments = window(text, start, text.len());
        assert_eq!(segments.last().map(|s| s.highlighted), Some(true));
    }

    #[test]
    fn long_line_is_cut_at_word_boundaries_with_ellipses() {
        let left = "лево ".repeat(30);
        let right = " право".repeat(30);
        let text = format!("{left}[[цель]]{right}\nвторая строка");
        let start = text.find("[[").unwrap();
        let segments = window(&text, start, start + "[[цель]]".len());

        assert_eq!(highlighted(&segments), vec!["[[цель]]"]);
        let shown = plain(&segments);
        assert!(shown.starts_with('…') && shown.ends_with('…'), "{shown}");
        assert!(!shown.contains("вторая"));
        // Слова не разрезаны: между многоточием и текстом целые слова.
        assert!(
            shown.starts_with("…лево") && shown.ends_with("право…"),
            "{shown}"
        );
        let before = segments.first().unwrap().text.chars().count();
        assert!(before <= WINDOW_CHARS + 1, "{before}");
    }

    #[test]
    fn only_the_own_occurrence_is_highlighted() {
        let text = "[[A]] и снова [[A]] в одной строке";
        let second = text.rfind("[[A]]").unwrap();
        let segments = window(text, second, second + 5);
        assert_eq!(highlighted(&segments), vec!["[[A]]"]);
        assert_eq!(segments.first().unwrap().text, "[[A]] и снова ");
        assert!(!segments.first().unwrap().highlighted);
    }

    #[test]
    fn multiline_occurrence_shows_the_first_line() {
        let text = "до [[длинное\nназвание]] после";
        let start = text.find("[[").unwrap();
        let end = text.find("]]").unwrap() + 2;
        let segments = window(text, start, end);
        assert_eq!(highlighted(&segments), vec!["[[длинное"]);
        assert_eq!(plain(&segments), "до [[длинное");
    }
}
