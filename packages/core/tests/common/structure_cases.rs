//! Строение текста и оглавление заметки — общая таблица для обоих таргетов
//! (фича 015).
//!
//! Функции чистые, платформенно нейтральные: расхождение между native и
//! браузером было бы дефектом (принцип II). Файлы `structure_native.rs` и
//! `structure_wasm.rs` только вызывают эти функции своим harness'ом.

#![allow(dead_code)]

use typsidian_core::preview::anchor::{locate_heading, HeadingProbe};
use typsidian_core::preview::{compile_note, document_headings, FontSet, TextRun};
use typsidian_core::structure::{
    carry_collapsed, heading_probe, outline, outline_active, outline_view, parse, FigureKind,
    LabelOwner, NoteStructure, Outline, OutlineKind, OutlineKinds, OutlineRequest, OutlineRow,
    OutlineState, SourcePlace,
};

// ── Помощники ───────────────────────────────────────────────────────────────

fn heads(text: &str) -> Vec<(u8, String)> {
    parse(text)
        .headings
        .into_iter()
        .map(|heading| (heading.level, heading.text))
        .collect()
}

fn pair(level: u8, text: &str) -> (u8, String) {
    (level, text.to_string())
}

/// Кусок текста по месту в кодовых единицах UTF-16.
fn slice(text: &str, place: &SourcePlace) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16(&units[place.from as usize..place.to as usize]).unwrap()
}

/// Смещение первого вхождения `needle` в UTF-16.
fn at(text: &str, needle: &str) -> u32 {
    let byte = text
        .find(needle)
        .unwrap_or_else(|| panic!("в тексте нет {needle:?}"));
    text[..byte].encode_utf16().count() as u32
}

fn request(kinds: OutlineKinds, filter: &str, caret: Option<u32>) -> OutlineRequest {
    OutlineRequest {
        kinds,
        collapsed: Vec::new(),
        filter: filter.to_string(),
        caret,
    }
}

fn plain_request() -> OutlineRequest {
    request(OutlineKinds::default(), "", None)
}

fn text_of(row: &OutlineRow) -> String {
    row.segments.iter().map(|part| part.text.as_str()).collect()
}

fn texts(view_rows: &[OutlineRow]) -> Vec<String> {
    view_rows.iter().map(text_of).collect()
}

/// Ключ первого пункта с таким текстом.
fn key_of(outline: &Outline, text: &str) -> String {
    outline
        .items
        .iter()
        .find(|item| item.text == text)
        .unwrap_or_else(|| panic!("в оглавлении нет {text:?}"))
        .key
        .clone()
}

fn highlighted(row: &OutlineRow) -> Vec<&str> {
    row.segments
        .iter()
        .filter(|part| part.highlighted)
        .map(|part| part.text.as_str())
        .collect()
}

const BOTH: OutlineKinds = OutlineKinds {
    figures: true,
    labels: true,
};
const FIGURES: OutlineKinds = OutlineKinds {
    figures: true,
    labels: false,
};
const LABELS: OutlineKinds = OutlineKinds {
    figures: false,
    labels: true,
};

// ── T006. Разбор ────────────────────────────────────────────────────────────

/// Заголовки знаками `=` и вызовом `heading(…)`, уровни не подряд.
pub fn headings_by_signs_and_by_call() {
    let text = "= Один\n#heading(level: 2)[Два]\n#heading[Без уровня]\n=== Три\n";
    assert_eq!(
        heads(text),
        vec![
            pair(1, "Один"),
            pair(2, "Два"),
            pair(1, "Без уровня"),
            pair(3, "Три"),
        ]
    );
    let found = parse(text);
    // Запись знаками начинается с `=`, вызов - с `#`.
    let first = found.headings[0].place;
    assert_eq!((first.line, first.column, first.from), (1, 1, 0));
    assert_eq!(slice(text, &first), "= Один");
    let call = found.headings[1].place;
    assert_eq!((call.line, call.column), (2, 1));
    assert_eq!(slice(text, &call), "#heading(level: 2)[Два]");
    assert!(found.headings.iter().all(|h| !h.text_is_source));
}

/// Уровень вызова: литерал `level`, сумма `offset` и `depth`, иначе 1.
pub fn call_levels_follow_the_documented_rule() {
    for (text, level) in [
        ("#heading(level: 3)[X]", 3),
        ("#heading(depth: 3)[X]", 3),
        ("#heading(offset: 2, depth: 2)[X]", 4),
        ("#heading(offset: 1)[X]", 2),
        ("#heading(level: auto, depth: 2)[X]", 2),
        // Уровень выражением, а не литералом, даёт 1.
        ("#let n = 4\n#heading(level: n)[X]", 1),
        ("#heading(level: 0)[X]", 1),
    ] {
        assert_eq!(heads(text), vec![pair(level, "X")], "{text}");
    }
}

/// `=` в коде, комментарии, строке и сыром блоке заголовком не считается.
pub fn equals_inside_code_comments_and_strings_is_not_a_heading() {
    let text = "// = комментарий\n/* = блок */\n#let s = \"= строка\"\n#{\n  let x = 1\n  // = ещё\n}\n`= сырое`\n= Настоящий\n";
    assert_eq!(heads(text), vec![pair(1, "Настоящий")]);
}

/// Заголовки, которых в тексте заметки нет: функция, цикл, правило, чужой файл.
pub fn computed_and_foreign_headings_are_skipped() {
    let text = "\
#let f(x) = [= В функции]
#let g = (x) => [= В безымянной]
#for i in range(2) [= В цикле]
#while false [= В while]
#show heading: it => [= В show]
#set heading(numbering: \"1.\")
#include \"a.typ\"
#import \"b.typ\": *
= Живой
";
    assert_eq!(heads(text), vec![pair(1, "Живой")]);
}

/// Блок содержимого аргумента и ветка `#if` - заголовок записан в самой заметке.
pub fn headings_inside_content_blocks_and_branches_are_found() {
    let text = "#align(center)[= В блоке]\n#if true [\n== В ветке\n] else [\n=== В else\n]\n";
    assert_eq!(
        heads(text),
        vec![pair(1, "В блоке"), pair(2, "В ветке"), pair(3, "В else")]
    );
}

/// Читаемый текст: оформление, ссылка, формула, пустой остаток.
pub fn readable_text_drops_markup() {
    assert_eq!(
        heads("= Привет, *мир* и _все_\n= `код` и https://typst.app\n= см. @intro\n"),
        vec![
            pair(1, "Привет, мир и все"),
            pair(1, "код и https://typst.app"),
            pair(1, "см. @intro"),
        ]
    );
    assert_eq!(
        heads("= #emph[важно] и #text(red)[A]\n= О $L^2$-нормах\n= a\\#b   c\n"),
        vec![
            pair(1, "важно и A"),
            pair(1, "О L^2-нормах"),
            pair(1, "a#b c")
        ]
    );

    // Читать нечего: показывается начало исходной записи.
    let found = parse("= #image(\"a.png\")\n");
    assert!(found.headings[0].text_is_source);
    assert_eq!(found.headings[0].text, "image(\"a.png\")");

    // Запасной текст обрезан до 60 знаков.
    let long = format!("= #image(\"{}.png\")\n", "ы".repeat(100));
    let found = parse(&long);
    assert!(found.headings[0].text_is_source);
    assert_eq!(found.headings[0].text.chars().count(), 60);
}

/// Рисунок и таблица: подпись и вид.
pub fn figures_and_tables_carry_kind_and_caption() {
    let text = "\
#figure(image(\"a.png\"), caption: [Схема *стенда*]) <fig:a>
#figure(table(columns: 2)[a][b], caption: [Данные])
#figure(table[x], kind: table)
#figure(rect(), caption: \"Строка\", kind: table)
#figure(image(\"x.png\"))
";
    let found = parse(text);
    let facts: Vec<(FigureKind, Option<&str>, Option<&str>)> = found
        .figures
        .iter()
        .map(|figure| {
            (
                figure.kind,
                figure.caption.as_deref(),
                figure.label.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        facts,
        vec![
            (FigureKind::Figure, Some("Схема стенда"), Some("fig:a")),
            (FigureKind::Table, Some("Данные"), None),
            (FigureKind::Table, None, None),
            (FigureKind::Table, Some("Строка"), None),
            (FigureKind::Figure, None, None),
        ]
    );
    assert_eq!(
        slice(text, &found.figures[0].place),
        "#figure(image(\"a.png\"), caption: [Схема *стенда*])"
    );
}

/// Метка при заголовке, при рисунке, отдельная; употребления в коде не в счёт.
pub fn labels_attach_to_the_previous_neighbour_and_code_uses_are_not_definitions() {
    let text = "\
= Заголовок <h1>

<сирота>

#figure(rect(), caption: [Ц]) <f1>
Текст <в-тексте>
#cite(<k>) #link(<a>)[x] #ref(<b>)
";
    let found = parse(text);
    let labels: Vec<(&str, LabelOwner)> = found
        .labels
        .iter()
        .map(|label| (label.name.as_str(), label.attached))
        .collect();
    assert_eq!(
        labels,
        vec![
            ("h1", LabelOwner::Heading),
            ("сирота", LabelOwner::Other),
            ("f1", LabelOwner::Figure),
            ("в-тексте", LabelOwner::Other),
        ]
    );
    assert_eq!(found.headings[0].label.as_deref(), Some("h1"));
    assert_eq!(found.figures[0].label.as_deref(), Some("f1"));
    assert_eq!(slice(text, &found.labels[0].place), "<h1>");

    // Метка на следующей строке привязана, через разрыв абзаца - нет.
    assert_eq!(parse("= З\n<м>").labels[0].attached, LabelOwner::Heading);
    assert_eq!(parse("= З\n\n<м>").labels[0].attached, LabelOwner::Other);
    // Вторая метка у того же заголовка к нему уже не привязана.
    let two = parse("= З <a> <b>");
    assert_eq!(two.labels[0].attached, LabelOwner::Heading);
    assert_eq!(two.labels[1].attached, LabelOwner::Other);
    assert_eq!(two.headings[0].label.as_deref(), Some("a"));
    // Метка заголовка-вызова.
    assert_eq!(
        parse("#heading[Х] <hx>").labels[0].attached,
        LabelOwner::Heading
    );
}

/// Ссылки `@имя`, с дополнением.
pub fn references_carry_target_and_place() {
    let text = "См. @intro и @fig:a[рис.] ещё.";
    let found = parse(text);
    let targets: Vec<&str> = found.refs.iter().map(|r| r.target.as_str()).collect();
    assert_eq!(targets, vec!["intro", "fig:a"]);
    assert_eq!(slice(text, &found.refs[0].place), "@intro");
    assert_eq!(slice(text, &found.refs[1].place), "@fig:a[рис.]");
}

/// Места на кириллице и символах вне BMP: строка и колонка в знаках, `from` и
/// `to` в кодовых единицах UTF-16.
pub fn places_count_characters_and_utf16_units() {
    let text = "Привет 😀 мир\n== Заголовок 😀\nа 😀 @ссылка\n";
    let found = parse(text);
    let heading = found.headings[0].place;
    assert_eq!((heading.line, heading.column), (2, 1));
    assert_eq!(
        heading.from,
        "Привет 😀 мир\n".encode_utf16().count() as u32
    );
    assert_eq!(slice(text, &heading), "== Заголовок 😀");

    // Колонка считает знаки (эмодзи - один), смещение - кодовые единицы (две).
    let reference = found.refs[0].place;
    assert_eq!((reference.line, reference.column), (3, 5));
    assert_eq!(
        reference.to - reference.from,
        "@ссылка".chars().count() as u32
    );
    assert_eq!(slice(text, &reference), "@ссылка");
}

/// Любой обрыв записи - без паники и с осмысленными местами.
pub fn broken_text_never_panics() {
    let sample = "= Заголовок 😀 <м>\n#heading(level: 2)[Текст *жирный* $x^2$]\n\
                  #figure(table[x], caption: [Ц <п>]) <ф>\n@ссылка[доп] #cite(<к>)\n\
                  #let f(x) = [= в]\n#if true [== в ветке]\n";
    let mut cuts: Vec<usize> = sample.char_indices().map(|(at, _)| at).collect();
    cuts.push(sample.len());
    for cut in cuts {
        let found: NoteStructure = parse(&sample[..cut]);
        let end = sample[..cut].encode_utf16().count() as u32;
        let places = found
            .headings
            .iter()
            .map(|h| h.place)
            .chain(found.figures.iter().map(|f| f.place))
            .chain(found.labels.iter().map(|l| l.place))
            .chain(found.refs.iter().map(|r| r.place));
        for place in places {
            assert!(
                place.from <= place.to && place.to <= end,
                "{cut}: {place:?}"
            );
            assert!(place.line >= 1 && place.column >= 1);
        }
        // Оглавление по такому же тексту строится тоже.
        let _ = outline_view(&outline(&sample[..cut]), &plain_request());
    }
    for rough in [
        "#heading(",
        "#figure(",
        "<",
        "@",
        "= ",
        "=",
        "#heading(level: 2)[",
        "$ x",
        "\u{0}",
        "",
    ] {
        let _ = parse(rough);
    }
}

// ── T009. Оглавление и вид ──────────────────────────────────────────────────

/// Порядок, уровни, родители при уровнях не подряд.
pub fn outline_follows_text_order_with_levels_and_parents() {
    let tree = outline("= A\n== B\n=== C\n== D\n= E\n=== F\n");
    let shape: Vec<(&str, u8, Option<u32>)> = tree
        .items
        .iter()
        .map(|item| (item.text.as_str(), item.level, item.parent))
        .collect();
    assert_eq!(
        shape,
        vec![
            ("A", 1, None),
            ("B", 2, Some(0)),
            ("C", 3, Some(1)),
            ("D", 2, Some(0)),
            ("E", 1, None),
            // После первого уровня сразу третий: родитель - E, отступ третьего.
            ("F", 3, Some(4)),
        ]
    );

    // Первый заголовок глубже следующего: родителя у обоих нет.
    let rising = outline("== Глубокий\n= Верхний\n");
    assert_eq!(rising.items[0].parent, None);
    assert_eq!(rising.items[1].parent, None);
}

/// Ключи уникальны, даже у одинаковых заголовков; место - начало записи.
pub fn outline_keys_are_unique_and_places_point_at_the_record() {
    let text = "= Тест\n= Тест\n== Тест\n#heading(level: 2)[Тест]\n";
    let tree = outline(text);
    let mut keys: Vec<&str> = tree.items.iter().map(|item| item.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), 4);

    assert_eq!(tree.items[0].place.from, 0);
    assert_eq!(tree.items[3].place.from, at(text, "#heading"));
    assert_eq!(tree.items[3].place.line, 4);

    // Правка текста в теле раздела ключей не меняет.
    let edited = outline("= Тест\nновая строка\n= Тест\n== Тест\n#heading(level: 2)[Тест]\n");
    let before: Vec<&str> = tree.items.iter().map(|i| i.key.as_str()).collect();
    let after: Vec<&str> = edited.items.iter().map(|i| i.key.as_str()).collect();
    assert_eq!(before, after);
}

/// Вид: заголовки по умолчанию, счётчики, состояние без заголовков и при ошибке.
pub fn view_shows_headings_with_counts_and_states() {
    let text = "= A\n== B\n#figure(rect(), caption: [Ц]) <ф>\n<м>\n";
    let view = outline_view(&outline(text), &plain_request());
    assert_eq!(texts(&view.rows), vec!["A", "B"]);
    assert_eq!(view.state, OutlineState::Ready);
    assert_eq!(
        (
            view.counts.headings,
            view.counts.figures,
            view.counts.tables,
            view.counts.labels
        ),
        (2, 1, 0, 1)
    );
    assert!(view.rows.iter().all(|row| row.label.is_none()));
    assert_eq!(view.active, None);

    let none = outline_view(&outline("просто текст\n"), &plain_request());
    assert!(none.rows.is_empty());
    assert_eq!(none.state, OutlineState::NoHeadings);

    // Текст с ошибкой разбора: всё, что распознано, остаётся.
    let broken = outline_view(
        &outline("= До\n#heading(level: 2)[Не закрыт\n"),
        &plain_request(),
    );
    assert!(texts(&broken.rows).contains(&"До".to_string()));
    assert_eq!(broken.state, OutlineState::Ready);
}

// ── T010. Соответствие собранному документу (SC-005) ───────────────────────

/// Заметки, где заголовки записаны в тексте, собираются настоящим компилятором,
/// и уровни с текстами совпадают с `outline`.
pub fn outline_matches_the_compiled_document() {
    let notes = [
        "= Один\n== Два\n=== Три\n= Снова один\n",
        "#heading(level: 2)[Функция]\n#heading[Без уровня]\n#heading(depth: 3)[Глубина]\n#heading(offset: 1)[Сдвиг]\n",
        "= С *жирным* и _курсивом_\n= Со `кодом`\n= Через #emph[вызов]\n= Слитно a\\#b   c\n",
        "#align(center)[= В блоке]\n#box[== Глубже]\n#if true [\n=== В ветке\n]\n",
        "= Заголовок <метка>\n== Другой\n<вторая>\n",
        "Вступление без заголовка.\n\n= Кириллица и 😀\nТекст.\n== Диапазон 1--2\n",
        "#let f(x) = [= В функции]\n#for i in range(2) [= В цикле #i]\n= Живой\n",
    ];
    for (number, note) in notes.iter().enumerate() {
        let from_text: Vec<(usize, String)> = outline(note)
            .items
            .iter()
            .filter(|item| item.kind == OutlineKind::Heading)
            .map(|item| (item.level as usize, item.text.clone()))
            .collect();
        let document = document_headings("note.typ", note, FontSet::builtin())
            .unwrap_or_else(|| panic!("заметка {number} не собралась"));
        let from_document: Vec<(usize, String)> = document
            .into_iter()
            .map(|heading| (heading.level, heading.text))
            .collect();
        if number == notes.len() - 1 {
            // Заголовки из цикла и функции создаёт вычисление: их нет в тексте
            // заметки, и оглавление их не показывает (граница из research.md §4).
            assert_eq!(from_text, vec![(1, "Живой".to_string())]);
            assert!(from_document.contains(&(1, "Живой".to_string())));
            assert!(from_document.len() > from_text.len());
        } else {
            assert_eq!(from_text, from_document, "заметка {number}:\n{note}");
        }
    }
}

// ── T011. Заголовок в предпросмотре ─────────────────────────────────────────

fn run(text: &str, size_pt: f64) -> TextRun {
    TextRun {
        text: text.to_string(),
        x_pt: 0.0,
        y_pt: 0.0,
        width_pt: 10.0,
        size_pt,
        lang: None,
    }
}

fn probe(text: &str, occurrence: u32) -> HeadingProbe {
    HeadingProbe {
        text: text.to_string(),
        occurrence,
    }
}

/// Проба: номер среди одинаковых, `None` для не-заголовков и исходной записи.
pub fn heading_probe_numbers_equal_headings() {
    let tree = outline(
        "= Итог\n= Другой\n= Итог\n= #image(\"a.png\")\n#figure(rect(), caption: [Итог])\n",
    );
    let item = |index: usize| tree.items[index].key.clone();
    assert_eq!(heading_probe(&tree, &item(0)), Some(probe("Итог", 0)));
    assert_eq!(heading_probe(&tree, &item(2)), Some(probe("Итог", 1)));
    // Исходная запись, рисунок и неизвестный ключ - без пробы.
    assert_eq!(heading_probe(&tree, &item(3)), None);
    assert_eq!(heading_probe(&tree, &item(4)), None);
    assert_eq!(heading_probe(&tree, "нет-такого"), None);
}

/// Поиск по прогонам, собранным руками.
pub fn locate_heading_on_hand_made_runs() {
    let found = |runs: &[Vec<TextRun>], probe: &HeadingProbe| {
        locate_heading(runs, probe).map(|a| (a.page_index, a.run_index, a.exact))
    };

    // Один прогон.
    let runs = vec![vec![run("текст", 11.0), run("Введение", 20.0)]];
    assert_eq!(found(&runs, &probe("Введение", 0)), Some((0, 1, true)));

    // Оформление разбило заголовок на прогоны.
    let runs = vec![vec![
        run("Вве", 20.0),
        run("дение", 20.0),
        run("текст", 11.0),
    ]];
    assert_eq!(found(&runs, &probe("Введение", 0)), Some((0, 0, true)));

    // Перенос строки: пробелы на стыке значения не имеют.
    let runs = vec![vec![run("Очень длинный", 20.0), run("заголовок", 20.0)]];
    assert_eq!(
        found(&runs, &probe("Очень длинный заголовок", 0)),
        Some((0, 0, true))
    );

    // Строка оглавления документа тем же текстом, но обычным кеглем.
    let runs = vec![
        vec![run("Введение", 11.0)],
        vec![run("x", 11.0), run("Введение", 20.0)],
    ];
    assert_eq!(found(&runs, &probe("Введение", 0)), Some((1, 1, true)));

    // Два одинаковых заголовка: по номеру; номер за концом - последний.
    let runs = vec![vec![run("Итог", 20.0)], vec![run("Итог", 20.0)]];
    assert_eq!(found(&runs, &probe("Итог", 0)), Some((0, 0, true)));
    assert_eq!(found(&runs, &probe("Итог", 1)), Some((1, 0, true)));
    assert_eq!(found(&runs, &probe("Итог", 9)), Some((1, 0, true)));

    // Нет совпадения, лишние прогоны в склейке, пустая проба.
    assert_eq!(found(&runs, &probe("Другой", 0)), None);
    let runs = vec![vec![run("Вве", 20.0), run("лишнее", 20.0)]];
    assert_eq!(found(&runs, &probe("Введение", 0)), None);
    assert_eq!(found(&runs, &probe("   ", 0)), None);
    assert_eq!(found(&[], &probe("Введение", 0)), None);
}

/// Поиск на документе, собранном настоящим компилятором, с оглавлением документа
/// и двумя заголовками одного текста.
pub fn locate_heading_on_a_compiled_document() {
    let text = "#outline()\n\n= Введение\nтекст\n#pagebreak()\n== Подробности\nещё\n#pagebreak()\n= Введение\nконец\n";
    let document = compile_note("note.typ", text, 1, FontSet::builtin()).unwrap();
    let runs: Vec<Vec<TextRun>> = document.pages.iter().map(|p| p.text_runs.clone()).collect();
    let tree = outline(text);

    let mut places = Vec::new();
    for item in &tree.items {
        let probe = heading_probe(&tree, &item.key).expect("проба заголовка");
        let anchor = locate_heading(&runs, &probe).unwrap_or_else(|| panic!("{}", item.text));
        let run = &runs[anchor.page_index as usize][anchor.run_index as usize];
        assert_eq!(run.text, item.text);
        // Не строка оглавления документа: заголовок набран крупнее.
        assert!(run.size_pt > 11.0, "{} - {}", item.text, run.size_pt);
        places.push((anchor.page_index, anchor.run_index));
    }
    // Два «Введения» указывают на разные места.
    assert_ne!(places[0], places[2]);
}

// ── T024. Текущий раздел ────────────────────────────────────────────────────

const SECTIONS: &str = "Вступление\n= A\nтекст\n== B\nещё текст\n== C\nконец\n";

/// Каретка выше первого заголовка, на нём, внутри, в конце и за концом.
pub fn the_active_section_follows_the_caret() {
    let tree = outline(SECTIONS);
    let (a, b, c) = (key_of(&tree, "A"), key_of(&tree, "B"), key_of(&tree, "C"));
    let end = SECTIONS.encode_utf16().count() as u32;
    let cases: Vec<(Option<u32>, Option<&String>)> = vec![
        (None, None),
        (Some(0), None),
        (Some(at(SECTIONS, "= A") - 1), None),
        // Начало строки заголовка - уже в нём.
        (Some(at(SECTIONS, "= A")), Some(&a)),
        (Some(at(SECTIONS, "текст")), Some(&a)),
        (Some(at(SECTIONS, "== B")), Some(&b)),
        (Some(at(SECTIONS, "ещё")), Some(&b)),
        (Some(at(SECTIONS, "== C") + 2), Some(&c)),
        (Some(end), Some(&c)),
        (Some(end + 1000), Some(&c)),
    ];
    for (caret, expected) in cases {
        let req = request(OutlineKinds::default(), "", caret);
        assert_eq!(
            outline_active(&tree, &req).as_ref(),
            expected,
            "каретка {caret:?}"
        );
        // Лёгкий ответ совпадает с видом.
        assert_eq!(
            outline_view(&tree, &req).active.as_ref(),
            expected,
            "каретка {caret:?}"
        );
    }
}

// ── T031. Свёрнутость и фильтр ──────────────────────────────────────────────

const NESTED: &str = "= Введение\n== Цели\n=== Подцели\n== Задачи\n= Заключение\n";

/// Потомки свёрнутого скрыты, неизвестный ключ игнорируется, родитель строки.
pub fn collapsed_branches_hide_their_descendants() {
    let tree = outline(NESTED);
    let intro = key_of(&tree, "Введение");
    let mut req = plain_request();
    req.collapsed = vec![intro.clone(), "ключа-нет".to_string()];
    let view = outline_view(&tree, &req);
    assert_eq!(texts(&view.rows), vec!["Введение", "Заключение"]);
    assert!(view.rows[0].collapsed && view.rows[0].has_children);
    assert!(!view.rows[1].collapsed && !view.rows[1].has_children);

    // Свёрнута только вложенная ветка: родитель строки - видимый предок.
    req.collapsed = vec![key_of(&tree, "Цели")];
    let view = outline_view(&tree, &req);
    assert_eq!(
        texts(&view.rows),
        vec!["Введение", "Цели", "Задачи", "Заключение"]
    );
    assert_eq!(view.rows[1].parent.as_deref(), Some(intro.as_str()));
    assert_eq!(view.rows[0].parent, None);
}

/// Фильтр: регистр, кириллица, разметка совпадения, предки с `dimmed`.
pub fn the_filter_keeps_matches_with_their_ancestors() {
    let tree = outline(NESTED);

    let view = outline_view(&tree, &request(OutlineKinds::default(), "ВВЕД", None));
    assert_eq!(texts(&view.rows), vec!["Введение"]);
    assert_eq!(highlighted(&view.rows[0]), vec!["Введ"]);
    assert!(!view.rows[0].dimmed);

    let view = outline_view(&tree, &request(OutlineKinds::default(), "  цели ", None));
    assert_eq!(texts(&view.rows), vec!["Введение", "Цели", "Подцели"]);
    assert!(view.rows[0].dimmed && highlighted(&view.rows[0]).is_empty());
    assert!(!view.rows[1].dimmed && !view.rows[2].dimmed);
    assert_eq!(highlighted(&view.rows[1]), vec!["Цели"]);
    assert_eq!(highlighted(&view.rows[2]), vec!["цели"]);
    assert_eq!(
        view.rows[2].parent.as_deref(),
        Some(view.rows[1].key.as_str())
    );

    // Фильтр из одних пробелов - фильтра нет.
    let blank = outline_view(&tree, &request(OutlineKinds::default(), "   ", None));
    let none = outline_view(&tree, &plain_request());
    assert_eq!(blank, none);

    let nothing = outline_view(&tree, &request(OutlineKinds::default(), "яяя", None));
    assert!(nothing.rows.is_empty());
    assert_eq!(nothing.state, OutlineState::NoMatches);
    assert_eq!(nothing.counts.headings, 5);
}

/// Свёрнутость не действует при фильтре и возвращается после его очистки.
pub fn collapsing_is_ignored_while_filtering() {
    let tree = outline(NESTED);
    let intro = key_of(&tree, "Введение");
    let mut req = request(OutlineKinds::default(), "подцели", None);
    req.collapsed = vec![intro];
    let view = outline_view(&tree, &req);
    assert_eq!(texts(&view.rows), vec!["Введение", "Цели", "Подцели"]);
    assert!(view.rows.iter().all(|row| !row.collapsed));

    req.filter = String::new();
    let view = outline_view(&tree, &req);
    assert_eq!(texts(&view.rows), vec!["Введение", "Заключение"]);
    assert!(view.rows[0].collapsed);
}

/// Текущий раздел в свёрнутой ветке и под фильтром - ближайший видимый предок.
pub fn the_active_section_climbs_to_a_visible_ancestor() {
    let tree = outline(NESTED);
    let intro = key_of(&tree, "Введение");
    let caret = Some(at(NESTED, "=== Подцели"));

    let mut req = request(OutlineKinds::default(), "", caret);
    req.collapsed = vec![intro.clone()];
    assert_eq!(outline_active(&tree, &req), Some(intro.clone()));
    assert_eq!(outline_view(&tree, &req).active, Some(intro.clone()));

    // Каретка в «Задачи», фильтр оставил «Введение» (как родителя) и «Цели».
    let caret = Some(at(NESTED, "== Задачи"));
    let req = request(OutlineKinds::default(), "цели", caret);
    assert_eq!(outline_active(&tree, &req), Some(intro));
    assert_eq!(
        outline_view(&tree, &req).active.as_deref(),
        outline_active(&tree, &req).as_deref()
    );

    // Нет видимого предка - подсветки нет.
    let caret = Some(at(NESTED, "= Заключение"));
    let req = request(OutlineKinds::default(), "цели", caret);
    assert_eq!(outline_active(&tree, &req), None);
    assert_eq!(outline_view(&tree, &req).active, None);
}

// ── T032. Перенос свёрнутости ───────────────────────────────────────────────

/// Правка текста, переименование, смена уровня, вставка и удаление заголовка.
pub fn carry_collapsed_follows_edits() {
    let prev = outline("= А\n== Б\n= В\n");
    let a = key_of(&prev, "А");

    // Правка текста внутри раздела: ключи те же.
    let same = outline("= А\nновая строка\n== Б\n= В\n");
    assert_eq!(
        carry_collapsed(&prev, &same, std::slice::from_ref(&a)),
        vec![a.clone()]
    );

    // Переименование: число заголовков то же - ключ переезжает.
    let renamed = outline("= А2\n== Б\n= В\n");
    assert_eq!(
        carry_collapsed(&prev, &renamed, std::slice::from_ref(&a)),
        vec![key_of(&renamed, "А2")]
    );

    // Смена уровня.
    let deeper = outline("== А\n== Б\n= В\n");
    assert_eq!(
        carry_collapsed(&prev, &deeper, std::slice::from_ref(&a)),
        vec![key_of(&deeper, "А")]
    );

    // Вставка заголовка выше: ключ не менялся - остаётся.
    let inserted = outline("= Новый\n= А\n== Б\n= В\n");
    assert_eq!(
        carry_collapsed(&prev, &inserted, std::slice::from_ref(&a)),
        vec![a.clone()]
    );

    // Переименование вместе со вставкой: число изменилось - свёрнутость снята.
    let both = outline("= Новый\n= А2\n== Б\n= В\n");
    assert!(carry_collapsed(&prev, &both, std::slice::from_ref(&a)).is_empty());

    // Удаление свёрнутого заголовка.
    let removed = outline("== Б\n= В\n");
    assert!(carry_collapsed(&prev, &removed, std::slice::from_ref(&a)).is_empty());

    // Чужие ключи и повторы не переходят.
    let carried = carry_collapsed(
        &prev,
        &renamed,
        &[a.clone(), a.clone(), "призрак".to_string()],
    );
    assert_eq!(carried, vec![key_of(&renamed, "А2")]);
}

// ── T040. Рисунки, таблицы и метки ──────────────────────────────────────────

const OBJECTS: &str = "\
Вводный текст <сирота-до>
= Раздел
#figure(image(\"a.png\"), caption: [Первый рисунок]) <рис1>
#figure(table[x], caption: [Первая таблица])
#figure(image(\"b.png\")) <безподписи>
== Подраздел <метка-подраздела>
Текст <просто>
#figure(image(\"c.png\"), caption: [Рисунок в подразделе])
";

/// Виды: по умолчанию только заголовки; рисунки и метки включаются порознь.
pub fn kinds_are_chosen_per_view() {
    let tree = outline(OBJECTS);

    let view = outline_view(&tree, &plain_request());
    assert_eq!(texts(&view.rows), vec!["Раздел", "Подраздел"]);
    assert!(view.rows.iter().all(|row| row.label.is_none()));

    // Рисунки и таблицы: под своим разделом, уровень раздела + 1.
    let view = outline_view(&tree, &request(FIGURES, "", None));
    assert_eq!(
        texts(&view.rows),
        vec![
            "Раздел",
            "Первый рисунок",
            "Первая таблица",
            "Подраздел",
            "Рисунок в подразделе"
        ]
    );
    let shape: Vec<(OutlineKind, u8)> = view.rows.iter().map(|r| (r.kind, r.level)).collect();
    assert_eq!(
        shape,
        vec![
            (OutlineKind::Heading, 1),
            (OutlineKind::Figure, 2),
            (OutlineKind::Table, 2),
            (OutlineKind::Heading, 2),
            (OutlineKind::Figure, 3),
        ]
    );
    assert_eq!(
        view.rows[1].parent.as_deref(),
        Some(view.rows[0].key.as_str())
    );
    assert!(view.rows[0].has_children);
    // Рисунок без подписи пунктом не становится.
    assert!(!texts(&view.rows).iter().any(|t| t.contains("безподписи")));

    // Метки: отдельные - пунктами, при заголовке - полем строки.
    let view = outline_view(&tree, &request(LABELS, "", None));
    assert_eq!(
        texts(&view.rows),
        vec!["сирота-до", "Раздел", "безподписи", "Подраздел", "просто"]
    );
    assert_eq!(view.rows[0].kind, OutlineKind::Label);
    assert_eq!((view.rows[0].level, view.rows[0].parent.clone()), (1, None));
    assert_eq!(view.rows[3].label.as_deref(), Some("метка-подраздела"));
    // Метка рисунка без подписи - пункт-метка под разделом.
    assert_eq!(
        (view.rows[2].kind, view.rows[2].level),
        (OutlineKind::Label, 2)
    );
    assert_eq!(
        (view.rows[4].kind, view.rows[4].level),
        (OutlineKind::Label, 3)
    );

    // Оба вида: метка рисунка показана при нём и отдельным пунктом не идёт.
    let view = outline_view(&tree, &request(BOTH, "", None));
    let figure = view
        .rows
        .iter()
        .find(|row| text_of(row) == "Первый рисунок")
        .unwrap();
    assert_eq!(figure.label.as_deref(), Some("рис1"));
    assert!(!texts(&view.rows).contains(&"рис1".to_string()));
}

/// Счётчики не зависят от видов; `has_children` считается по включённым видам.
pub fn counts_cover_the_whole_note_and_children_follow_the_kinds() {
    let tree = outline(OBJECTS);
    for kinds in [OutlineKinds::default(), FIGURES, LABELS, BOTH] {
        let counts = outline_view(&tree, &request(kinds, "", None)).counts;
        assert_eq!(
            (
                counts.headings,
                counts.figures,
                counts.tables,
                counts.labels
            ),
            (2, 2, 1, 3)
        );
    }
    // Пока виды выключены, вложенные есть только у «Раздела» (его «Подраздел»).
    let view = outline_view(&tree, &plain_request());
    assert!(view.rows[0].has_children);
    assert!(!view.rows[1].has_children);
    let view = outline_view(&tree, &request(FIGURES, "", None));
    assert!(view.rows[3].has_children);
}

/// Фильтр ищет и по имени метки; текущим разделом рисунок или метка не бывают.
pub fn filter_reaches_label_names_and_the_active_section_is_always_a_heading() {
    let tree = outline(OBJECTS);

    let view = outline_view(&tree, &request(LABELS, "метка-под", None));
    assert_eq!(texts(&view.rows), vec!["Раздел", "Подраздел"]);
    assert!(view.rows[0].dimmed && !view.rows[1].dimmed);
    // Совпадение в метке, а не в тексте: подсветки в тексте нет.
    assert!(highlighted(&view.rows[1]).is_empty());

    // Без показа меток по их имени не ищется.
    let view = outline_view(&tree, &request(OutlineKinds::default(), "метка-под", None));
    assert_eq!(view.state, OutlineState::NoMatches);

    // Пункт-метка находится по имени, совпадение размечено.
    let view = outline_view(&tree, &request(LABELS, "ПРОСТО", None));
    assert_eq!(texts(&view.rows), vec!["Раздел", "Подраздел", "просто"]);
    assert_eq!(highlighted(&view.rows[2]), vec!["просто"]);

    // Каретка на строке рисунка - текущий раздел его заголовок.
    let caret = Some(at(OBJECTS, "#figure(image(\"c.png\")"));
    let req = request(BOTH, "", caret);
    let tree_key = key_of(&tree, "Подраздел");
    assert_eq!(outline_active(&tree, &req), Some(tree_key));
}

// ── Метки для дополнения 018 (T004, T005) ──────────────────────────────────

/// Вид, текст и «верхний уровень» каждой метки: то, что 018 показывает в
/// списке и по чему решает о повторах.
pub fn labels_carry_kind_text_and_top_level() {
    use typsidian_core::structure::LabelKind;
    let text = "= Введение <введение>\n\n\
                #figure(rect(), caption: [Схема установки]) <рис-схема>\n\n\
                #figure(table(columns: 2)[a][b], caption: [Замеры]) <таб-замеры>\n\n\
                $ E = m c^2 $ <eq-энергия>\n\n\
                Просто метка <просто>\n\n\
                #if true [ = Вариант <вариант> ] else [ = Иначе <вариант> ]\n\n\
                #let тело = [Текст <в-let>]\n\n\
                #[ Блок <в-блоке> ]\n";
    let labels = parse(text).labels;
    let found: Vec<(&str, LabelKind, &str, bool)> = labels
        .iter()
        .map(|label| {
            (
                label.name.as_str(),
                label.kind,
                label.text.as_str(),
                label.top_level,
            )
        })
        .collect();
    assert_eq!(
        found,
        vec![
            ("введение", LabelKind::Heading, "Введение", true),
            ("рис-схема", LabelKind::Figure, "Схема установки", true),
            ("таб-замеры", LabelKind::Table, "Замеры", true),
            ("eq-энергия", LabelKind::Equation, "$ E = m c^2 $", true),
            ("просто", LabelKind::Other, "", true),
            ("вариант", LabelKind::Heading, "Вариант", false),
            ("вариант", LabelKind::Heading, "Иначе", false),
            ("в-let", LabelKind::Other, "", false),
            // Блок содержимого в разметке вычисляется один раз.
            ("в-блоке", LabelKind::Other, "", true),
        ]
    );
    // Оглавление 015 видит прежнюю привязку: формула — не рисунок и не заголовок.
    let equation = labels
        .iter()
        .find(|label| label.name == "eq-энергия")
        .unwrap();
    assert_eq!(equation.attached, LabelOwner::Other);
}
