//! Режим показа слайдов — общая таблица для обоих таргетов (фича 019).
//!
//! Сбор сведений о страницах идёт через настоящую сборку (`compile_note`),
//! разметку `<pdfpc>` тесты пишут руками — пакеты презентаций и сеть не нужны.
//! Правила колоды проверяются на записях `PageSlideInfo`, собранных вручную:
//! они чистые и от вёрстки не зависят. Файлы `slides_native.rs` и
//! `slides_wasm.rs` только вызывают эти функции своим harness'ом (принцип II).

#![allow(dead_code)]

use typsidian_core::messages::AppMessage;
use typsidian_core::preview::{
    compile_note, compile_note_resolved, CompileOutcome, CompiledDocument, FontSet, PackageCache,
};
use typsidian_core::slides::{
    caret_at_page, carry, navigate, show_keymap, slide_label, start, PageSlideInfo, ShowAction,
    ShowState, SlideLabel, SlidePlace, SlidesError,
};

use super::MemoryVault;

const MAIN: &str = "заметка.typ";

// ── Помощники ───────────────────────────────────────────────────────────────

fn build(text: &str) -> CompiledDocument {
    let document = compile_note(MAIN, text, 1, FontSet::builtin()).unwrap();
    assert_eq!(
        document.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        document.diagnostics
    );
    document
}

/// Смещение первого вхождения `needle` в UTF-16 — так его считает редактор.
fn at(text: &str, needle: &str) -> u32 {
    let byte = text.find(needle).expect("образец есть в тексте");
    text[..byte].encode_utf16().count() as u32
}

fn offsets(document: &CompiledDocument) -> Vec<Option<u32>> {
    document
        .slides
        .iter()
        .map(|s| s.source_start.map(|p| p.offset))
        .collect()
}

/// Слайды вручную: пары «смещение текста, логический номер».
fn deck(spec: &[(Option<u32>, Option<u32>)]) -> Vec<PageSlideInfo> {
    spec.iter()
        .enumerate()
        .map(|(index, (offset, logical))| PageSlideInfo {
            page_index: index as u32,
            source_start: offset.map(|offset| SlidePlace {
                offset,
                line: offset / 10,
                column: offset % 10,
            }),
            logical_slide: *logical,
            notes: None,
        })
        .collect()
}

/// Колода из `n` страниц без логических номеров, текст с 10-го знака через 10.
fn plain_deck(n: u32) -> Vec<PageSlideInfo> {
    let spec: Vec<_> = (0..n).map(|i| (Some(10 + i * 10), None)).collect();
    deck(&spec)
}

fn state(page: u32, total: u32) -> ShowState {
    ShowState {
        page,
        total,
        started_at_ms: 1000.0,
    }
}

fn label(
    current: u32,
    total: u32,
    step: Option<u32>,
    steps: Option<u32>,
    next: Option<u32>,
) -> SlideLabel {
    SlideLabel {
        current,
        total,
        step,
        steps,
        next_page: next,
    }
}

const THREE: &str = "#set page(width: 16cm, height: 9cm)\n\
Первый слайд\n#pagebreak()\nВторой слайд\n#pagebreak()\nТретий слайд\n";

// ── Сбор при сборке ─────────────────────────────────────────────────────────

/// Три страницы — три записи по порядку, `source_start` возрастает и указывает
/// на начало текста страницы.
pub fn three_pages_give_rising_source_starts() {
    let document = build(THREE);
    assert_eq!(document.pages.len(), 3);
    assert_eq!(document.slides.len(), document.pages.len());
    for (index, slide) in document.slides.iter().enumerate() {
        assert_eq!(slide.page_index, index as u32);
        assert_eq!(slide.page_index as usize, document.pages[index].index);
    }
    assert_eq!(
        offsets(&document),
        [
            Some(at(THREE, "Первый")),
            Some(at(THREE, "Второй")),
            Some(at(THREE, "Третий")),
        ]
    );
    // Строка и колонка — с нуля: «Первый слайд» на второй строке, с начала.
    let first = document.slides[0].source_start.unwrap();
    assert_eq!((first.line, first.column), (1, 0));
    let third = document.slides[2].source_start.unwrap();
    assert_eq!((third.line, third.column), (5, 0));
}

/// Кириллица и знаки вне BMP до текста: смещение считается в UTF-16, строка и
/// колонка — по тексту.
pub fn source_start_counts_utf16_units() {
    let text = "// 😀𝔘 комментарий до текста\n\
#set page(width: 16cm, height: 9cm)\n\
#text(red)[Слайд]\n";
    let document = build(text);
    let place = document.slides[0].source_start.expect("текст на странице");
    let expected = at(text, "Слайд");
    assert_eq!(place.offset, expected);
    // Байтовое смещение и счёт знаков дают другие числа: проверка не вырожденная.
    assert_ne!(expected as usize, text.find("Слайд").unwrap());
    assert_ne!(
        expected as usize,
        text[..text.find("Слайд").unwrap()].chars().count()
    );
    assert_eq!((place.line, place.column), (2, "#text(red)[".len() as u32));
}

/// Страница без текста заметки (фигура) и страница с текстом из `#include`
/// получают `None`.
pub fn pages_without_own_text_have_no_source_start() {
    let text = "#set page(width: 16cm, height: 9cm)\n\
Первый слайд\n#pagebreak()\n#rect(width: 2cm, height: 2cm)\n#pagebreak()\nТретий слайд\n";
    let document = build(text);
    assert_eq!(
        offsets(&document),
        [Some(at(text, "Первый")), None, Some(at(text, "Третий"))]
    );
}

/// Текст страницы пришёл из другого файла — своего текста у страницы нет.
pub async fn an_included_page_has_no_source_start() {
    let vault = MemoryVault::new().with_file("чужой.typ", "Чужой текст слайда\n");
    let text = "#set page(width: 16cm, height: 9cm)\n\
Первый слайд\n#pagebreak()\n#include \"чужой.typ\"\n";
    let document = compile_note_resolved(
        &vault,
        &PackageCache::new(),
        MAIN,
        text,
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();
    assert_eq!(document.outcome, CompileOutcome::Succeeded);
    assert_eq!(document.slides.len(), 2);
    assert_eq!(offsets(&document), [Some(at(text, "Первый")), None]);
}

/// Заметка строкой есть только у своей страницы; две заметки склеиваются через
/// пустую строку; содержимое отдаёт простой текст.
pub fn notes_are_attached_to_their_pages() {
    let text = "#set page(width: 16cm, height: 9cm)\n\
Первый слайд\n#pagebreak()\n\
Второй слайд\n\
#metadata((t: \"Note\", v: \"Скажи про первое\")) <pdfpc>\n\
#metadata((t: \"Note\", v: [*жирно*])) <pdfpc>\n\
#pagebreak()\n\
Третий слайд\n";
    let document = build(text);
    let notes: Vec<Option<&str>> = document.slides.iter().map(|s| s.notes.as_deref()).collect();
    assert_eq!(notes, [None, Some("Скажи про первое\n\nжирно"), None]);

    let single = build(
        "#set page(width: 16cm, height: 9cm)\nСлайд\n\
#metadata((t: \"Note\", v: [*жирно*])) <pdfpc>\n",
    );
    assert_eq!(single.slides[0].notes.as_deref(), Some("жирно"));
}

/// Логические номера из `<pdfpc>` ложатся на страницы; чужие виды записей и
/// значения не того типа пропускаются.
pub fn logical_slides_come_from_the_markup() {
    let mut text = String::from("#set page(width: 16cm, height: 9cm)\n");
    for (i, number) in [1, 1, 2, 3].into_iter().enumerate() {
        if i > 0 {
            text.push_str("#pagebreak()\n");
        }
        text.push_str(&format!(
            "Страница {i}\n#metadata((t: \"LogicalSlide\", v: {number})) <pdfpc>\n"
        ));
    }
    let document = build(&text);
    let logical: Vec<Option<u32>> = document.slides.iter().map(|s| s.logical_slide).collect();
    assert_eq!(logical, [Some(1), Some(1), Some(2), Some(3)]);

    // Мусор в разметке не мешает сборке и не даёт записей.
    let junk = build(
        "#set page(width: 16cm, height: 9cm)\nСлайд\n\
#metadata((t: \"LogicalSlide\", v: \"один\")) <pdfpc>\n\
#metadata((t: \"Note\", v: 42)) <pdfpc>\n\
#metadata((t: \"Idx\", v: 1)) <pdfpc>\n\
#metadata(\"просто строка\") <pdfpc>\n\
#metadata((v: 1)) <pdfpc>\n\
#metadata((t: \"LogicalSlide\", v: -3)) <pdfpc>\n",
    );
    assert_eq!(junk.slides[0].logical_slide, None);
    assert_eq!(junk.slides[0].notes, None);
}

/// Без разметки `<pdfpc>` у страниц нет ни заметок, ни номеров слайдов, и
/// подписи идут по страницам.
pub fn a_document_without_markup_is_numbered_by_pages() {
    let document = build(THREE);
    assert!(document
        .slides
        .iter()
        .all(|s| s.notes.is_none() && s.logical_slide.is_none()));
    assert_eq!(
        slide_label(&document.slides, 1),
        label(2, 3, None, None, Some(2))
    );
}

/// Сбор не трогает страницы: две сборки одного текста дают те же отпечатки, а у
/// неудавшейся сборки сведений о слайдах нет, как и страниц. Поле отсутствует в
/// ответе старого вида — читается как пустое.
pub fn collecting_does_not_change_pages() {
    let one = build(THREE);
    let two = build(THREE);
    let digests = |d: &CompiledDocument| -> Vec<String> {
        d.pages.iter().map(|p| p.digest.clone()).collect()
    };
    assert_eq!(digests(&one), digests(&two));
    assert_eq!(one.pages.len(), 3);

    let failed = compile_note(MAIN, "#let x = (\n", 1, FontSet::builtin()).unwrap();
    assert_eq!(failed.outcome, CompileOutcome::Failed);
    assert!(failed.pages.is_empty());
    assert!(failed.slides.is_empty());

    let mut json = serde_json::to_value(&one).unwrap();
    assert!(json.as_object_mut().unwrap().remove("slides").is_some());
    let old: CompiledDocument = serde_json::from_value(json).unwrap();
    assert!(old.slides.is_empty());
    assert_eq!(old.pages.len(), 3);
}

// ── Запуск ──────────────────────────────────────────────────────────────────

/// Каретка во второй странице — страница 1; до первого текста — 0; после всего —
/// последняя. Без каретки — первая.
pub fn start_picks_the_slide_under_the_caret() {
    let document = build(THREE);
    let slides = &document.slides;
    let second = at(THREE, "Второй");

    assert_eq!(
        start(slides, Some(second), 5.0).unwrap(),
        state(1, 3).with_start(5.0)
    );
    // В середине текста второго слайда.
    assert_eq!(start(slides, Some(second + 4), 0.0).unwrap().page, 1);
    // Перед первым текстом, у самого начала заметки.
    assert_eq!(start(slides, Some(0), 0.0).unwrap().page, 0);
    // После всего текста.
    assert_eq!(start(slides, Some(u32::MAX), 0.0).unwrap().page, 2);
    assert_eq!(start(slides, None, 0.0).unwrap().page, 0);
}

/// Пошаговый слайд — несколько страниц с одним и тем же текстом: каретка
/// ведёт на первый шаг.
pub fn a_stepped_slide_starts_at_its_first_step() {
    let text = "#set page(width: 16cm, height: 9cm)\n\
Вступление\n#pagebreak()\n\
#for i in range(3) [Шаг слайда #pagebreak(weak: true)]\n\
Конец\n";
    let document = build(text);
    assert_eq!(document.slides.len(), 5);
    let step = Some(at(text, "Шаг"));
    assert_eq!(
        offsets(&document),
        [
            Some(at(text, "Вступление")),
            step,
            step,
            step,
            Some(at(text, "Конец"))
        ]
    );
    let caret = at(text, "Шаг") + 2;
    assert_eq!(start(&document.slides, Some(caret), 0.0).unwrap().page, 1);
    assert_eq!(
        caret_at_page(&document.slides, 3),
        document.slides[1].source_start
    );
}

/// Пустая колода показу не годится, и ошибка превращается в сообщение
/// приложения.
pub fn an_empty_deck_is_an_error() {
    assert_eq!(start(&[], None, 0.0), Err(SlidesError::EmptyDeck));
    assert_eq!(start(&[], Some(3), 0.0), Err(SlidesError::EmptyDeck));
    assert_eq!(
        SlidesError::EmptyDeck.message(),
        AppMessage::SlidesEmptyDeck
    );
    assert!(!SlidesError::EmptyDeck.message().log_text().is_empty());
    let json = serde_json::to_value(SlidesError::EmptyDeck).unwrap();
    assert_eq!(json, serde_json::json!({ "kind": "EmptyDeck" }));
}

/// При равных смещениях и первой странице без текста правило «наибольшее
/// не больше каретки» не путается.
pub fn caret_before_any_text_and_pages_without_text() {
    let slides = deck(&[
        (None, None),
        (Some(20), None),
        (None, None),
        (Some(40), None),
    ]);
    assert_eq!(start(&slides, Some(5), 0.0).unwrap().page, 0);
    assert_eq!(start(&slides, Some(20), 0.0).unwrap().page, 1);
    assert_eq!(start(&slides, Some(39), 0.0).unwrap().page, 1);
    assert_eq!(start(&slides, Some(40), 0.0).unwrap().page, 3);
}

// ── Листание ────────────────────────────────────────────────────────────────

pub fn navigate_stops_at_the_borders() {
    let slides = plain_deck(3);
    let go = |page, action| navigate(state(page, 3), &slides, action).page;
    assert_eq!(go(0, ShowAction::Next), 1);
    assert_eq!(go(2, ShowAction::Next), 2);
    assert_eq!(go(2, ShowAction::Previous), 1);
    assert_eq!(go(0, ShowAction::Previous), 0);
    assert_eq!(go(1, ShowAction::First), 0);
    assert_eq!(go(1, ShowAction::Last), 2);
    assert_eq!(go(0, ShowAction::GoToPage { index: 2 }), 2);
    assert_eq!(go(0, ShowAction::GoToPage { index: 99 }), 2);
    // Время начала листание не трогает.
    let moved = navigate(state(0, 3), &slides, ShowAction::Next);
    assert_eq!(moved.started_at_ms, 1000.0);
    // Число страниц берётся из колоды, а не из устаревшего состояния.
    assert_eq!(navigate(state(1, 9), &slides, ShowAction::Next).total, 3);
}

pub fn go_to_slide_without_logical_numbers_counts_pages() {
    let slides = plain_deck(5);
    let go = |number| navigate(state(0, 5), &slides, ShowAction::GoToSlide { number }).page;
    assert_eq!(go(3), 2);
    assert_eq!(go(1), 0);
    assert_eq!(go(0), 0);
    assert_eq!(go(99), 4);
}

pub fn go_to_slide_with_logical_numbers_finds_the_first_page() {
    // Слайды 1, 1, 2, 3 на четырёх страницах.
    let slides = deck(&[
        (Some(10), Some(1)),
        (Some(10), Some(1)),
        (Some(20), Some(2)),
        (Some(30), Some(3)),
    ]);
    let go = |number| navigate(state(3, 4), &slides, ShowAction::GoToSlide { number }).page;
    assert_eq!(go(1), 0);
    assert_eq!(go(2), 2);
    assert_eq!(go(3), 3);
    assert_eq!(go(99), 3);
    assert_eq!(go(0), 0);

    // Пропущенный номер ведёт на следующий слайд.
    let gap = deck(&[
        (Some(10), Some(1)),
        (Some(20), Some(2)),
        (Some(30), Some(4)),
    ]);
    assert_eq!(
        navigate(state(0, 3), &gap, ShowAction::GoToSlide { number: 3 }).page,
        2
    );
}

/// Логические номера, которым нельзя верить (есть пробелы или убывание),
/// не используются: слайды считаются по страницам.
pub fn broken_logical_numbers_fall_back_to_pages() {
    let decreasing = deck(&[
        (Some(10), Some(2)),
        (Some(20), Some(1)),
        (Some(30), Some(3)),
    ]);
    assert_eq!(
        slide_label(&decreasing, 1),
        label(2, 3, None, None, Some(2))
    );
    assert_eq!(
        navigate(
            state(0, 3),
            &decreasing,
            ShowAction::GoToSlide { number: 2 }
        )
        .page,
        1
    );
    let partial = deck(&[(Some(10), Some(1)), (Some(20), None), (Some(30), Some(2))]);
    assert_eq!(slide_label(&partial, 2), label(3, 3, None, None, None));
}

// ── Подписи ─────────────────────────────────────────────────────────────────

pub fn labels_show_steps_of_logical_slides() {
    let slides = deck(&[
        (Some(10), Some(1)),
        (Some(10), Some(1)),
        (Some(20), Some(2)),
        (Some(30), Some(3)),
    ]);
    // Вторая страница первого слайда: слайд 1 из 3, шаг 2 из 2, дальше — страница 2.
    assert_eq!(
        slide_label(&slides, 1),
        label(1, 3, Some(2), Some(2), Some(2))
    );
    assert_eq!(
        slide_label(&slides, 0),
        label(1, 3, Some(1), Some(2), Some(1))
    );
    // У слайда с одним шагом шагов нет.
    assert_eq!(slide_label(&slides, 2), label(2, 3, None, None, Some(3)));
    // На последней странице «дальше» пусто.
    assert_eq!(slide_label(&slides, 3), label(3, 3, None, None, None));
    // Страница за пределами колоды прижимается к последней.
    assert_eq!(slide_label(&slides, 99), slide_label(&slides, 3));
}

pub fn labels_without_markup_count_pages() {
    let slides = plain_deck(4);
    assert_eq!(slide_label(&slides, 0), label(1, 4, None, None, Some(1)));
    assert_eq!(slide_label(&slides, 3), label(4, 4, None, None, None));
    assert_eq!(slide_label(&[], 0), label(0, 0, None, None, None));
}

// ── Перенос через пересборку ────────────────────────────────────────────────

pub fn carry_keeps_the_page_and_clamps_to_the_last() {
    let same = carry(state(2, 5), &plain_deck(5)).unwrap();
    assert_eq!(same, state(2, 5));
    // Страниц стало больше: страница та же, число новое.
    assert_eq!(carry(state(2, 5), &plain_deck(8)).unwrap(), state(2, 8));
    // Меньше текущего номера: последняя страница.
    assert_eq!(carry(state(4, 5), &plain_deck(3)).unwrap(), state(2, 3));
    // Время начала не сбрасывается.
    assert_eq!(
        carry(state(4, 5), &plain_deck(3)).unwrap().started_at_ms,
        1000.0
    );
}

pub fn carry_to_no_pages_is_an_error() {
    assert_eq!(carry(state(1, 5), &[]), Err(SlidesError::EmptyDeck));
}

// ── Каретка на выходе ───────────────────────────────────────────────────────

pub fn caret_at_page_falls_back_to_the_previous_text() {
    let slides = deck(&[
        (None, None),
        (Some(10), None),
        (None, None),
        (Some(30), None),
    ]);
    let place = |offset: u32| SlidePlace {
        offset,
        line: offset / 10,
        column: offset % 10,
    };
    // Страница без текста в начале и до него: места нет.
    assert_eq!(caret_at_page(&slides, 0), None);
    assert_eq!(caret_at_page(&slides, 1), Some(place(10)));
    // Страница-картинка — начало предыдущей.
    assert_eq!(caret_at_page(&slides, 2), Some(place(10)));
    assert_eq!(caret_at_page(&slides, 3), Some(place(30)));
    // За пределами колоды — как на последней странице.
    assert_eq!(caret_at_page(&slides, 99), Some(place(30)));
    assert_eq!(caret_at_page(&[], 0), None);
}

/// Страница-картинка в собранном документе ведёт каретку к предыдущей.
pub fn caret_at_an_image_page_is_the_previous_start() {
    let text = "#set page(width: 16cm, height: 9cm)\n\
Первый слайд\n#pagebreak()\n#rect(width: 2cm, height: 2cm)\n";
    let document = build(text);
    assert_eq!(document.slides.len(), 2);
    assert_eq!(document.slides[1].source_start, None);
    assert_eq!(
        caret_at_page(&document.slides, 1),
        document.slides[0].source_start
    );
}

// ── Клавиши и форма данных ──────────────────────────────────────────────────

pub fn the_keymap_has_clicker_and_arrow_keys() {
    let map = show_keymap();
    let find = |key: &str| {
        map.bindings
            .iter()
            .find(|binding| binding.key == key)
            .map(|binding| binding.action)
    };
    for key in ["ArrowRight", "ArrowDown", " ", "PageDown", "Enter"] {
        assert_eq!(find(key), Some(ShowAction::Next), "{key}");
    }
    for key in ["ArrowLeft", "ArrowUp", "PageUp", "Backspace"] {
        assert_eq!(find(key), Some(ShowAction::Previous), "{key}");
    }
    assert_eq!(find("Home"), Some(ShowAction::First));
    assert_eq!(find("End"), Some(ShowAction::Last));
    assert_eq!(find("Escape"), None);
    // Клавиши в таблице не повторяются.
    let mut keys: Vec<&str> = map.bindings.iter().map(|b| b.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), map.bindings.len());
}

/// Действия идут через границу как `{ kind, … }` с именами в camelCase.
pub fn actions_cross_the_boundary_as_tagged_objects() {
    use serde_json::json;
    let cases = [
        (ShowAction::Next, json!({ "kind": "next" })),
        (ShowAction::Previous, json!({ "kind": "previous" })),
        (ShowAction::First, json!({ "kind": "first" })),
        (ShowAction::Last, json!({ "kind": "last" })),
        (
            ShowAction::GoToSlide { number: 2 },
            json!({ "kind": "goToSlide", "number": 2 }),
        ),
        (
            ShowAction::GoToPage { index: 4 },
            json!({ "kind": "goToPage", "index": 4 }),
        ),
    ];
    for (action, expected) in cases {
        let json = serde_json::to_value(action).unwrap();
        assert_eq!(json, expected);
        assert_eq!(serde_json::from_value::<ShowAction>(json).unwrap(), action);
    }
}

trait WithStart {
    fn with_start(self, started_at_ms: f64) -> Self;
}

impl WithStart for ShowState {
    fn with_start(mut self, started_at_ms: f64) -> Self {
        self.started_at_ms = started_at_ms;
        self
    }
}
