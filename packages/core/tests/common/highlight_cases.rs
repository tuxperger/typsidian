//! Подсветка Typst — общая таблица для обоих таргетов (фича 024).
//!
//! Функция чистая и платформенно нейтральная: расхождение между native и
//! браузером было бы дефектом (принцип II, FR-010). Файлы `highlight_native.rs`
//! и `highlight_wasm.rs` только вызывают эти функции своим harness'ом.
//!
//! Проверки идут «по позициям»: у каждой единицы UTF-16 текста — роль и
//! начертание или ничего. Так тест не зависит от того, как соседние листья
//! склеились во фрагменты.

#![allow(dead_code)]

use typsidian_core::highlight::{
    highlight, HighlightEdit, HighlightSession, HighlightSpan, HighlightWindow, StyleFlags,
    SyntaxRole,
};

use SyntaxRole::*;

// ── Образцы ────────────────────────────────────────────────────────────────

/// Текст артборда «Редактор» временного макета (plan.md → Макет).
pub const SAMPLE: &str = r#"// Глава 3. Результаты эксперимента
#import "../шаблоны/диссертация.typ": рисунок, таблица-данных
#set text(lang: "ru", size: 12pt)
#set par(justify: true, first-line-indent: 1.25em)
#show heading.where(level: 2): set block(above: 1.4em)

= Результаты
== Тепловой режим образца

Нагрев шёл _равномерно_, без локальных перегревов: см. @fig:термограмма.
Максимум отклонения — *3.2%* при нагрузке 450 Вт, а \#7 серии отбракован.
Схема — в «Методологии» и в "протоколе №4", исходники на https://gitlab.lab.ru/тепло.

- калибровка датчиков до и после серии
- шаг измерений `dt = 0.5 s`
+ прогрев 15 мин
+ запись 60 мин \
/ Термопара: датчик типа K, класс допуска 1.

$ Delta T = 1/n sum_(i=1)^n (T_i - T_0) $

#let k = calc.pow(2, 10) * 0.5  /* коэффициент из 2.3 */
#figure(
  image("рисунки/термограмма.png", width: 80%),
  caption: [Термограмма образца при *450 Вт*],
) <fig:термограмма>

#for (i, t) in данные.enumerate() [
  Замер #(i + 1): $T = #t$ °C, статус #none
]
"#;

// ── Помощники ──────────────────────────────────────────────────────────────

type Cell = Option<(SyntaxRole, StyleFlags)>;

fn spans(text: &str, windows: &[HighlightWindow]) -> Vec<HighlightSpan> {
    highlight(text, windows).spans()
}

/// Роль и начертание каждой единицы UTF-16 текста.
fn cells(text: &str, windows: &[HighlightWindow]) -> Vec<Cell> {
    let len = text.encode_utf16().count();
    let mut cells = vec![None; len];
    for span in spans(text, windows) {
        for cell in &mut cells[span.from..span.to] {
            *cell = Some((span.role, span.flags));
        }
    }
    cells
}

/// Диапазон UTF-16 `nth`-го (с нуля) вхождения `needle`.
fn find(text: &str, needle: &str, nth: usize) -> std::ops::Range<usize> {
    let byte = text
        .match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("нет вхождения №{nth} «{needle}»"))
        .0;
    let from = text[..byte].encode_utf16().count();
    from..from + needle.encode_utf16().count()
}

fn describe(cells: &[Cell], range: std::ops::Range<usize>) -> Vec<Cell> {
    cells[range].to_vec()
}

/// Каждая единица `nth`-го вхождения `needle` — роль `role` с начертанием `flags`.
#[track_caller]
fn expect_nth(text: &str, needle: &str, nth: usize, role: SyntaxRole, flags: StyleFlags) {
    let all = cells(text, &[]);
    let range = find(text, needle, nth);
    let got = describe(&all, range.clone());
    assert!(
        got.iter().all(|cell| *cell == Some((role, flags))),
        "«{needle}» (№{nth}): ждали {role:?}/{flags:?}, вышло {got:?}\nтекст: {text}"
    );
}

#[track_caller]
fn expect(text: &str, needle: &str, role: SyntaxRole) {
    expect_nth(text, needle, 0, role, StyleFlags::NONE);
}

/// У `needle` нет роли: обычный текст.
#[track_caller]
fn expect_plain(text: &str, needle: &str) {
    let all = cells(text, &[]);
    let got = describe(&all, find(text, needle, 0));
    assert!(
        got.iter().all(Option::is_none),
        "«{needle}» должен быть обычным текстом, вышло {got:?}\nтекст: {text}"
    );
}

/// Инварианты 1–3 data-model для ответа.
#[track_caller]
fn assert_invariants(text: &str, windows: &[HighlightWindow]) {
    let units: Vec<u16> = text.encode_utf16().collect();
    let found = spans(text, windows);
    let mut previous_end = 0;
    for span in &found {
        assert!(span.from < span.to, "пустой фрагмент {span:?}");
        assert!(
            span.from >= previous_end,
            "пересечение или беспорядок у {span:?}"
        );
        assert!(span.to <= units.len(), "фрагмент за концом текста {span:?}");
        for edge in [span.from, span.to] {
            let inside_pair =
                edge > 0 && edge < units.len() && (0xDC00..=0xDFFF).contains(&units[edge]);
            assert!(!inside_pair, "граница внутри суррогатной пары у {span:?}");
        }
        previous_end = span.to;
    }
    if !windows.is_empty() {
        for span in &found {
            assert!(
                windows.iter().any(|w| span.from < w.to && w.from < span.to),
                "фрагмент {span:?} не пересекает ни одно окно {windows:?}"
            );
        }
    }
}

fn window(from: usize, to: usize) -> HighlightWindow {
    HighlightWindow { from, to }
}

const STRONG: StyleFlags = StyleFlags::STRONG;
const EMPH: StyleFlags = StyleFlags::EMPH;
const HEADING: StyleFlags = StyleFlags::HEADING;

// ── Инварианты и окна (T006) ───────────────────────────────────────────────

pub fn invariants_hold_on_sample_and_edge_texts() {
    let long = format!(
        "#image.decode(\"{}\")\n= Приложение А\n",
        "A".repeat(100_000)
    );
    for text in [SAMPLE, "", "$", "Ж𝔸 *x*", "#", "/*", "`", long.as_str()] {
        assert_invariants(text, &[]);
    }
    let len = SAMPLE.encode_utf16().count();
    assert_invariants(SAMPLE, &[window(10, 40), window(300, 420)]);
    assert_invariants(SAMPLE, &[window(len, len + 50)]);
}

pub fn offsets_are_utf16_after_cyrillic_and_emoji() {
    let text = "Ж𝔸 ё #let x = 1";
    let at = find(text, "#let", 0);
    let first = spans(text, &[])
        .into_iter()
        .next()
        .expect("решётка должна получить роль");
    assert_eq!(first.from, at.start, "смещения — в единицах UTF-16");
    expect(text, "let", Keyword);
    expect(text, "1", Number);
}

pub fn span_crossing_window_edge_is_returned_whole() {
    let text = "#let s = \"длинная строка из многих слов\"";
    let string = find(text, "\"длинная", 0).start..text.encode_utf16().count();
    let middle = string.start + 5;
    let found = spans(text, &[window(middle, middle + 2)]);
    assert_eq!(found.len(), 1, "в окне одна строка: {found:?}");
    assert_eq!((found[0].from, found[0].to), (string.start, string.end));
    assert_eq!(found[0].role, String);
}

pub fn windows_union_matches_separate_windows() {
    let len = SAMPLE.encode_utf16().count();
    let a = window(0, len / 3);
    let b = window(len / 2, len);
    let whole = cells(SAMPLE, &[]);
    let joint = cells(SAMPLE, &[a, b]);
    let left = cells(SAMPLE, &[a]);
    let right = cells(SAMPLE, &[b]);
    for w in [a, b] {
        for at in w.from..w.to {
            assert_eq!(
                joint[at], whole[at],
                "окна не должны менять роль позиции {at}"
            );
            let separate = if at < a.to { left[at] } else { right[at] };
            assert_eq!(joint[at], separate, "позиция {at}");
        }
    }
}

pub fn empty_window_list_means_whole_text() {
    let len = SAMPLE.encode_utf16().count();
    assert_eq!(highlight(SAMPLE, &[]), highlight(SAMPLE, &[window(0, len)]));
}

// ── Разметка (T016, US1) ───────────────────────────────────────────────────

pub fn headings_mark_marker_and_text() {
    let text = "= Результаты\n\n== Тепловой режим\n\n=== Итог\n";
    expect_nth(text, "=", 0, HeadingMarker, HEADING);
    expect_nth(text, "Результаты", 0, Plain, HEADING);
    expect_nth(text, "==", 0, HeadingMarker, HEADING);
    expect_nth(text, "Тепловой режим", 0, Plain, HEADING);
    expect_nth(text, "===", 0, HeadingMarker, HEADING);
}

pub fn strong_and_emph_keep_their_markers() {
    let text = "Важно: *3.2%* и _равномерно_.";
    expect_nth(text, "*", 0, MarkupMarker, STRONG);
    expect_nth(text, "3.2%", 0, Plain, STRONG);
    expect_nth(text, "*", 1, MarkupMarker, STRONG);
    expect_nth(text, "_", 0, MarkupMarker, EMPH);
    expect_nth(text, "равномерно", 0, Plain, EMPH);
    expect_plain(text, "Важно");
}

pub fn list_markers_and_terms() {
    let text = "- калибровка\n+ прогрев\n1. запись\n/ Термопара: датчик типа K\n";
    expect(text, "-", ListMarker);
    expect(text, "+", ListMarker);
    expect(text, "1.", ListMarker);
    expect(text, "/", ListMarker);
    expect(text, "Термопара", ListTerm);
    expect_plain(text, "калибровка");
    expect_plain(text, "датчик типа K");
}

pub fn raw_content_is_not_markup() {
    let text = "шаг `dt = *0.5* s` и\n```rust\nlet x = \"#y\";\n```\n";
    expect_nth(text, "`", 0, MarkupMarker, StyleFlags::NONE);
    expect(text, "dt = *0.5* s", Raw);
    expect(text, "```", MarkupMarker);
    expect(text, "let x = \"#y\";", Raw);
}

pub fn labels_refs_links_and_escapes() {
    let text = "См. @рис:схема.1 ниже.\n#figure[x] <рис:схема.1>\nАдрес https://example.com/a*b*c и \\#7 \\\nконец";
    expect(text, "@рис:схема.1", Ref);
    expect(text, "<рис:схема.1>", Label);
    expect(text, "https://example.com/a*b*c", Link);
    expect(text, "\\#", Escape);
    expect_nth(text, "\\", 1, Escape, StyleFlags::NONE);
}

// ── Обычный текст остаётся текстом (T017, US1, FR-005) ─────────────────────

pub fn stray_symbols_do_not_recolor_text() {
    let text = "В \"протоколе №4\" и 2*3 = 6, слово*внутри, a = b, # и конец #\nСледующая строка.";
    expect_plain(text, "\"протоколе №4\"");
    expect_plain(text, "2*3 = 6");
    expect_plain(text, "a = b");
    expect_plain(text, "Следующая строка.");
}

/// Строки артборда «Было · стало», где старая подсветка ошибалась (SC-004).
pub fn old_highlighter_mistakes_in_markup_are_gone() {
    let text = "Схема — в \"протоколе №4\", исходники на https://gitlab.lab.ru.\nМаксимум — *3.2%* при 450 Вт\n";
    expect_plain(text, "\"протоколе №4\", исходники на ");
    expect(text, "https://gitlab.lab.ru", Link);
    expect_nth(text, "3.2%", 0, Plain, STRONG);
}

// ── Код (T021, US2) ────────────────────────────────────────────────────────

pub fn keywords_are_keywords() {
    let text = "#let x = 1\n#set text(lang: \"ru\")\n#show heading: it => it\n#if x > 0 [a] else [b]\n#for i in (1, 2) [#i]\n#while false { break }\n#import \"a.typ\": b\n#include \"c.typ\"\n";
    for keyword in [
        "let", "set", "show", "if", "else", "for", "while", "break", "import", "include",
    ] {
        expect(text, keyword, Keyword);
    }
    expect(text, "#", Keyword);
    // «in» встречается внутри «heading» — проверяется отдельным текстом.
    expect("#for i in (1, 2) [#i]", "in", Keyword);
}

pub fn call_with_named_args_and_content() {
    let text = "#figure(image(\"a.png\"), caption: [Схема *важна*])";
    expect(text, "figure", Function);
    expect(text, "image", Function);
    expect(text, "caption", ArgName);
    expect(text, "\"a.png\"", String);
    expect_plain(text, "Схема ");
    expect_nth(text, "важна", 0, Plain, STRONG);
}

pub fn callee_path_is_a_function_whole() {
    let text = "#calc.pow(2, 10)\n#text.with(size: 12pt)\n#show heading.where(level: 2): set block(above: 1em)\n#a.b.c(1)";
    expect(text, "calc.pow", Function);
    expect(text, "text.with", Function);
    expect(text, "heading.where", Function);
    expect(text, "a.b.c", Function);
    expect(text, "size", ArgName);
}

pub fn numbers_and_constants() {
    let text = "#(10, 2.5, 12pt, 1.5em, 50%, 90deg, 1fr, true, false, none, auto)";
    for number in ["10", "2.5", "12pt", "1.5em", "50%", "90deg", "1fr"] {
        expect(text, number, Number);
    }
    for constant in ["true", "false", "none", "auto"] {
        expect(text, constant, Constant);
    }
}

pub fn string_escapes_and_nested_comments() {
    let text = "#let s = \"он сказал \\\"да\\\"\"\n/* внешний /* внутренний */ всё ещё */ текст\n// строка\n";
    expect(text, "\"он сказал ", String);
    expect(text, "/* внешний /* внутренний */ всё ещё */", Comment);
    expect_plain(text, " текст");
    expect(text, "// строка", Comment);
}

pub fn multiline_call_stays_code() {
    let text = "#figure(\n  image(\"a.png\", width: 80%),\n  caption: [Подпись],\n) <fig:a>\n";
    expect(text, "image", Function);
    expect(text, "width", ArgName);
    expect(text, "80%", Number);
    expect(text, "caption", ArgName);
    expect(text, "<fig:a>", Label);
}

// ── Формулы и вложенность (T022, US2) ──────────────────────────────────────

pub fn math_parts_are_distinct() {
    let text = "$ a^2 + b^2 = c^2 $ и $ sum_(i=1)^n #x $";
    expect_nth(text, "$", 0, MathDelimiter, StyleFlags::NONE);
    expect(text, "a", MathIdent);
    expect(text, "^", MathOperator);
    expect(text, "sum", MathIdent);
    expect(text, "#", Keyword);
    expect(text, "x", Variable);
}

/// Строка 30 образца: разметка → код → разметка → формула → код.
pub fn nesting_three_levels_deep() {
    let text =
        "#for (i, t) in данные.enumerate() [\n  Замер #(i + 1): $T = #t$ °C, статус #none\n]";
    expect(text, "данные.enumerate", Function);
    expect_plain(text, "Замер ");
    expect_nth(text, "#", 1, Keyword, StyleFlags::NONE);
    expect(text, "+", Operator);
    expect(text, "1", Number);
    expect(text, "$", MathDelimiter);
    expect(text, "T", MathIdent);
    let inner = find(text, "#t$", 0).start;
    let all = cells(text, &[]);
    assert_eq!(
        all[inner],
        Some((Keyword, StyleFlags::NONE)),
        "решётка в формуле"
    );
    assert_eq!(
        all[inner + 1],
        Some((Variable, StyleFlags::NONE)),
        "код в формуле"
    );
    expect_plain(text, " °C, статус ");
    expect(text, "none", Constant);
}

/// Строки артборда «Было · стало» для кода и формул (SC-004).
pub fn old_highlighter_mistakes_in_code_are_gone() {
    let text =
        "#let k = calc.pow(2, 10) * 0.5\n$ sum_(i=1)^n (T_i - T_0) $\nшаг `dt = \"0.5 s\"` — raw\n";
    expect(text, "calc.pow", Function);
    expect(text, "sum", MathIdent);
    expect(text, "T", MathIdent);
    expect(text, "dt = \"0.5 s\"", Raw);
    expect_plain(text, " — raw");
}

// ── Устойчивость (T025, US3) ───────────────────────────────────────────────

const NEXT: &str = "\n\n== Обсуждение\nДанные — _предварительные_.";

#[track_caller]
fn next_paragraph_is_intact(prefix: &str) {
    let text = format!("{prefix}{NEXT}");
    expect_nth(&text, "==", 0, HeadingMarker, HEADING);
    expect_nth(&text, "Обсуждение", 0, Plain, HEADING);
    expect_nth(&text, "предварительные", 0, Plain, EMPH);
}

pub fn unclosed_pairs_stay_within_paragraph() {
    for prefix in [
        "#figure(image(\"схема.png\"",
        "#let s = \"не закрыта",
        "Это *важно",
        "Это _термин",
        "Формула $ a + b",
    ] {
        next_paragraph_is_intact(prefix);
    }
}

pub fn unclosed_comment_and_raw_run_to_the_end() {
    let text = format!("/* черновик{NEXT}");
    expect(&text, "Обсуждение", Comment);
    // Незакрытый raw у Typst — ошибка до конца текста: остаток без раскраски.
    let text = format!("```\nкод{NEXT}");
    expect_plain(&text, "Обсуждение");
}

pub fn colors_recover_after_an_error() {
    next_paragraph_is_intact("#let x = (1, 2 ]");
}

pub fn very_long_line_is_one_or_two_spans() {
    let text = format!("#image.decode(\"{}\")", "A".repeat(100_000));
    let long = spans(&text, &[])
        .into_iter()
        .filter(|span| span.to - span.from > 1_000)
        .count();
    assert_eq!(long, 1, "строка base64 — один фрагмент");
}

// ── Паритет (T029, US4) ────────────────────────────────────────────────────

/// Эталонный ответ для короткого образца. Совпадение на обоих таргетах — FR-010.
pub fn packed_answer_matches_reference() {
    let text = "= Итог\n#let k = calc.pow(2, 10) // ё\n$x^2$ и *да*";
    let got = highlight(text, &[]).packed;
    assert_eq!(got, REFERENCE.to_vec(), "ответ: {got:?}");
}

/// Сверено вручную: `=` маркер заголовка, «Итог» с начертанием заголовка,
/// `#let` ключевое, `calc.pow` функция целиком, `// ё` комментарий, формула,
/// `*да*` с маркерами.
const REFERENCE: [u32; 57] = [
    0, 1, 1028, 2, 6, 1046, 7, 11, 14, 14, 15, 16, 16, 24, 19, 24, 25, 1, 25, 26, 17, 26, 27, 1,
    28, 30, 17, 30, 31, 1, 32, 36, 0, 37, 38, 11, 38, 39, 13, 39, 40, 12, 40, 41, 13, 41, 42, 11,
    45, 46, 259, 46, 48, 278, 48, 49, 259,
];

// ── Сессия (research.md §2, запасной вариант SC-002) ───────────────────────

/// Та же правка над обычной строкой — эталон для текста сессии.
fn apply(text: &str, edit: &HighlightEdit) -> std::string::String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut out = std::string::String::from_utf16(&units[..edit.from]).unwrap();
    out.push_str(&edit.insert);
    out.push_str(&std::string::String::from_utf16(&units[edit.to..]).unwrap());
    out
}

fn edit(from: usize, to: usize, insert: &str) -> HighlightEdit {
    HighlightEdit {
        from,
        to,
        insert: insert.to_string(),
    }
}

/// После любой цепочки правок раскраска сессии — как разбор того же текста с
/// нуля (FR-007), и текст сессии — копия редактора.
pub fn session_matches_fresh_parse_after_edits() {
    let mut session = HighlightSession::new(SAMPLE);
    let mut text = SAMPLE.to_string();
    let middle = find(SAMPLE, "Нагрев", 0).start;
    let steps: Vec<Vec<HighlightEdit>> = vec![
        vec![edit(0, 0, "#figure(")],
        vec![edit(0, 8, "")],
        vec![edit(middle, middle, "#")],
        vec![edit(middle + 1, middle + 1, "let ё = \"строка")],
        vec![edit(middle, middle + 17, "")],
        // Две правки одной транзакции: сначала правая, затем левая — как шлёт клиент.
        vec![edit(middle + 6, middle + 6, "*"), edit(middle, middle, "_")],
        vec![edit(0, 3, "/*")],
        vec![edit(0, 2, "//")],
        vec![edit(5, 5, "Ж𝔸")],
    ];
    for (step, edits) in steps.iter().enumerate() {
        for e in edits {
            text = apply(&text, e);
        }
        let len = text.encode_utf16().count();
        let got = session
            .update(edits, len, &[])
            .unwrap_or_else(|| panic!("шаг {step}: сессия разошлась с текстом"));
        assert_eq!(session.text(), text, "шаг {step}: текст сессии");
        assert_eq!(
            got,
            highlight(&text, &[]),
            "шаг {step}: раскраска как с нуля"
        );
        let window = [window(len / 3, len / 2)];
        assert_eq!(
            session.highlight(&window),
            highlight(&text, &window),
            "шаг {step}: и для окна"
        );
    }
}

/// Расхождение с редактором распознаётся, а не превращается в чужую раскраску.
pub fn session_reports_desync() {
    let mut session = HighlightSession::new("= Итог\n");
    let len = "= Итог\n".encode_utf16().count();
    assert!(
        session.update(&[edit(0, 0, "x")], len, &[]).is_none(),
        "длина не сошлась"
    );
    let mut session = HighlightSession::new("= Итог\n");
    assert!(
        session.update(&[edit(5, 99, "")], 5, &[]).is_none(),
        "правка за концом"
    );
    let mut session = HighlightSession::new("= Итог\n");
    assert!(
        session.update(&[], len, &[]).is_some(),
        "пустая правка — не расхождение"
    );
}
