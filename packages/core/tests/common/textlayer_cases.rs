//! Текстовый слой над страницей — общая таблица для обоих таргетов
//! (фича 005, contracts/text-layer.md, T031-T034).
//!
//! Ошибка в этом слое не видна глазом: страница выглядит правильно, а
//! выделение, копирование и программа чтения с экрана работают неверно.
//! Поэтому проверки здесь описывают именно то, чего не видно на картинке, — а
//! то, что видно, проверяет `a_real_note_compiles_into_pages_with_svg`.
//!
//! Файловой системы ни одна проверка не касается: `compile_note` работает над
//! переданным текстом, поэтому правило одно на оба таргета и расхождение между
//! ними было бы дефектом (принцип II).

#![allow(dead_code)]

use typsidian_core::preview::{compile_note, CompileOutcome, FontSet, TextRun};

const MAIN: &str = "заметка.typ";

fn page_runs(source: &str, page: usize) -> Vec<TextRun> {
    let doc = compile_note(MAIN, source, 1, FontSet::builtin())
        .expect("заметка обязана собраться");
    assert_eq!(
        doc.outcome,
        CompileOutcome::Succeeded,
        "проверка текстового слоя имеет смысл только на собранном документе"
    );
    doc.pages
        .get(page)
        .unwrap_or_else(|| panic!("ожидали страницу {page}, а их {}", doc.pages.len()))
        .text_runs
        .clone()
}

fn find<'a>(runs: &'a [TextRun], needle: &str) -> &'a TextRun {
    runs.iter()
        .find(|run| run.text.contains(needle))
        .unwrap_or_else(|| {
            let seen: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
            panic!("прогона с {needle:?} нет в слое; слой: {seen:?}")
        })
}

fn index_of(runs: &[TextRun], needle: &str) -> usize {
    runs.iter()
        .position(|run| run.text.contains(needle))
        .unwrap_or_else(|| panic!("прогона с {needle:?} нет в слое"))
}

/// T031: прогон несёт текст, место, кегль и язык.
pub fn a_run_carries_its_text_place_size_and_language() {
    let runs = page_runs("#set text(lang: \"ru\", size: 17pt)\nПривет\n", 0);
    let run = find(&runs, "Привет");

    assert_eq!(run.lang.as_deref(), Some("ru"), "язык объявлен (FR-032)");
    assert!(
        (run.size_pt - 17.0).abs() < 0.01,
        "кегль берётся из вёрстки, а не из умолчания: {}",
        run.size_pt
    );
    assert!(
        run.width_pt > 0.0,
        "ширина считается по x_advance глифов, ноль означает, что её не посчитали"
    );
    assert!(
        run.x_pt > 0.0 && run.y_pt > 0.0,
        "прогон стоит внутри страницы, а не в начале координат: ({}, {})",
        run.x_pt,
        run.y_pt
    );
}

/// T032: прогон внутри группы с трансформацией приведён к координатам
/// страницы.
///
/// Самый вероятный дефект слоя — потерянный `group.transform`: картинка
/// остаётся правильной, а текст съезжает. Поворот на 180° вокруг **левого
/// верхнего угла** делает пропуск однозначно видимым: правильный ответ уходит
/// в отрицательные координаты, а пропуск оставляет прогон там же, где он лежал
/// внутри группы, — в положительных. Поля страницы обнулены, чтобы отступ не
/// сдвинул результат обратно в плюс и не спрятал ошибку.
pub fn a_run_inside_a_transformed_group_lands_in_page_coordinates() {
    let plain = page_runs("#set page(margin: 0pt)\n#place(top + left)[Повёрнуто]\n", 0);
    let upright = find(&plain, "Повёрнуто");
    assert!(
        upright.y_pt > 0.0,
        "без поворота базовая линия ниже верха страницы: {}",
        upright.y_pt
    );

    let rotated = page_runs(
        "#set page(margin: 0pt)\n#place(top + left, rotate(180deg, origin: top + left)[Повёрнуто])\n",
        0,
    );
    let run = find(&rotated, "Повёрнуто");
    assert!(
        run.y_pt < 0.0,
        "поворот вокруг левого верхнего угла обязан увести базовую линию \
         в отрицательную область; {} означает, что `group.transform` потерян",
        run.y_pt
    );
}

/// T033: порядок прогонов — порядок чтения, а не порядок обхода кадра.
///
/// Проверяется на двух раскладках сразу, потому что ломаются они по-разному:
/// в колонках вторая колонка лежит в кадре отдельной группой, а вторая
/// страница — вообще другим `RenderedPage`.
pub fn runs_follow_reading_order_across_columns_and_pages() {
    let columns = page_runs(
        "#set page(columns: 2)\nПервая\n#colbreak()\nВторая\n",
        0,
    );
    assert!(
        index_of(&columns, "Первая") < index_of(&columns, "Вторая"),
        "выделение через колонки обязано дать связный текст (SC-011)"
    );

    let source = "Раньше\n#pagebreak()\nПозже\n";
    let first = page_runs(source, 0);
    let second = page_runs(source, 1);
    find(&first, "Раньше");
    find(&second, "Позже");
    assert!(
        !first.iter().any(|run| run.text.contains("Позже")),
        "слой страницы не имеет права нести текст соседней страницы"
    );
}

/// T034: сгенерированное содержимое попадает в слой.
///
/// Номера страниц в исходнике заметки нет ни одного символа — он появляется
/// при вёрстке. FR-031 требует, чтобы копировался текст готового документа,
/// поэтому слой обязан взять его из кадра, а не из разметки.
pub fn generated_content_reaches_the_layer() {
    let source = "#set page(numbering: \"1\")\nПервая\n#pagebreak()\nВторая\n";
    assert!(
        !source.contains('2'),
        "предпосылка проверки: цифры 2 в исходнике нет"
    );

    let second = page_runs(source, 1);
    find(&second, "2");
}
