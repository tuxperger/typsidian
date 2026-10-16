//! Колода: запуск, листание, подписи и каретка на выходе (research.md §1, §4,
//! §5; data-model.md).
//!
//! Функции чистые и состояния не держат: `ShowState` приходит от окна
//! редактора и уходит ему же. Источник истины о числе страниц — `slides`, а не
//! `ShowState::total`: после пересборки состояние могло отстать.

use super::{PageSlideInfo, ShowAction, ShowState, SlideLabel, SlidePlace, SlidesError};

/// Запуск показа. С кареткой — на слайде под ней, без неё — на первой странице.
pub fn start(
    slides: &[PageSlideInfo],
    caret: Option<u32>,
    now_ms: f64,
) -> Result<ShowState, SlidesError> {
    if slides.is_empty() {
        return Err(SlidesError::EmptyDeck);
    }
    let page = caret.map_or(0, |caret| page_at_caret(slides, caret));
    Ok(ShowState {
        page,
        total: slides.len() as u32,
        started_at_ms: now_ms,
    })
}

/// Страница под кареткой: наибольший `source_start.offset ≤ caret`, при
/// равенстве — первая (первый шаг пошагового слайда). Каретка до первого
/// текста — первая страница.
fn page_at_caret(slides: &[PageSlideInfo], caret: u32) -> u32 {
    let mut best: Option<(u32, u32)> = None;
    for (index, slide) in slides.iter().enumerate() {
        let Some(place) = slide.source_start else {
            continue;
        };
        if place.offset <= caret && best.is_none_or(|(offset, _)| place.offset > offset) {
            best = Some((place.offset, index as u32));
        }
    }
    best.map_or(0, |(_, page)| page)
}

/// Выполняет действие листания. Границы не пересекаются: `Next` на последней
/// странице и `Previous` на первой ничего не меняют.
pub fn navigate(state: ShowState, slides: &[PageSlideInfo], action: ShowAction) -> ShowState {
    let total = slides.len() as u32;
    if total == 0 {
        return ShowState { total, ..state };
    }
    let last = total - 1;
    let current = state.page.min(last);
    let page = match action {
        ShowAction::Next => (current + 1).min(last),
        ShowAction::Previous => current.saturating_sub(1),
        ShowAction::First => 0,
        ShowAction::Last => last,
        ShowAction::GoToSlide { number } => page_of_slide(slides, number),
        ShowAction::GoToPage { index } => index.min(last),
    };
    ShowState {
        page,
        total,
        ..state
    }
}

/// Показ после пересборки: страница сохраняется по индексу, а если страниц
/// стало меньше — последняя. Привязки к тексту слайда нет (research.md §5).
pub fn carry(state: ShowState, slides: &[PageSlideInfo]) -> Result<ShowState, SlidesError> {
    if slides.is_empty() {
        return Err(SlidesError::EmptyDeck);
    }
    let total = slides.len() as u32;
    Ok(ShowState {
        page: state.page.min(total - 1),
        total,
        ..state
    })
}

/// Куда поставить каретку при выходе из показа: начало текста на странице, а у
/// страницы без текста — на ближайшей предыдущей. `None` — на странице и до
/// неё текста заметки нет.
pub fn caret_at_page(slides: &[PageSlideInfo], page: u32) -> Option<SlidePlace> {
    let end = (page as usize).saturating_add(1).min(slides.len());
    slides[..end].iter().rev().find_map(|s| s.source_start)
}

/// Подпись счётчиков. Страница за пределами колоды прижимается к последней.
pub fn slide_label(slides: &[PageSlideInfo], page: u32) -> SlideLabel {
    let total_pages = slides.len() as u32;
    if total_pages == 0 {
        return SlideLabel {
            current: 0,
            total: 0,
            step: None,
            steps: None,
            next_page: None,
        };
    }
    let page = page.min(total_pages - 1);
    let next_page = (page + 1 < total_pages).then_some(page + 1);

    if !has_logical_numbers(slides) {
        return SlideLabel {
            current: page + 1,
            total: total_pages,
            step: None,
            steps: None,
            next_page,
        };
    }
    let number = |index: usize| slides[index].logical_slide.unwrap_or(0);
    let current = number(page as usize);
    let steps = slides
        .iter()
        .filter(|s| s.logical_slide == Some(current))
        .count() as u32;
    let first = slides
        .iter()
        .position(|s| s.logical_slide == Some(current))
        .unwrap_or(0) as u32;
    SlideLabel {
        current,
        total: number(slides.len() - 1),
        step: (steps > 1).then_some(page - first + 1),
        steps: (steps > 1).then_some(steps),
        next_page,
    }
}

/// Можно ли доверять логическим номерам: они есть у каждой страницы и не
/// убывают (data-model.md «Правила»). Иначе нумеруются страницы.
fn has_logical_numbers(slides: &[PageSlideInfo]) -> bool {
    let mut previous = 0;
    for slide in slides {
        match slide.logical_slide {
            Some(n) if n >= previous => previous = n,
            _ => return false,
        }
    }
    true
}

/// Первая страница слайда с номером `number`. `0` — первая страница, номер
/// больше последнего — последняя; пропущенный номер ведёт на следующий.
fn page_of_slide(slides: &[PageSlideInfo], number: u32) -> u32 {
    let last = slides.len() as u32 - 1;
    if number == 0 {
        return 0;
    }
    if has_logical_numbers(slides) {
        slides
            .iter()
            .position(|s| s.logical_slide.is_some_and(|n| n >= number))
            .map_or(last, |index| index as u32)
    } else {
        (number - 1).min(last)
    }
}
