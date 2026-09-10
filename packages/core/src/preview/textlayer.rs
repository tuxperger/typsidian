//! Текстовый слой над страницей (contracts/text-layer.md, FR-031, FR-032).
//!
//! `typst_svg` выводит каждый глиф контуром `<path>` — элемента `<text>` в
//! выводе нет вовсе (`typst-svg-0.15.1/src/text.rs`). Значит из картинки
//! страницы нельзя ни выделить текст, ни скопировать его, ни прочитать
//! программой чтения с экрана. Отсюда второй слой: настоящий текст,
//! невидимый, позиционированный поверх изображения — как в просмотрщиках PDF.
//!
//! # Текст берётся из вёрстки, а не из исходника заметки
//!
//! Это не деталь реализации, а требование: после вёрстки текст документа
//! отличается от написанного — переносы, лигатуры, номера страниц в
//! колонтитуле, нумерация, оглавление. FR-031 требует, чтобы скопированный
//! фрагмент содержал текст **готового документа**, поэтому источник здесь —
//! `FrameItem::Text` свёрстанного кадра, и никакой другой.
//!
//! # Как считаются координаты
//!
//! Правило composition взято не на глаз, а из `typst-svg-0.15.1/src/lib.rs`
//! (`render_frame`, `render_group`), чтобы слой не разъезжался с картинкой —
//! самый вероятный дефект здесь:
//!
//! ```text
//! на каждый (pos, item):   T' = T ∘ translate(pos)
//! группа:                  вглубь с T'' = T' ∘ group.transform
//! прогон текста:           начало = (T'.tx, T'.ty)
//! ```
//!
//! Одно расхождение с `typst-svg` намеренное. Тот различает `FrameKind::Soft`
//! и `FrameKind::Hard`: для «жёсткого» кадра он сбрасывает состояние в
//! единичное и вешает составную матрицу атрибутом на `<g>`. Это приём
//! **сериализации** SVG, а не другая геометрия: итоговая матрица для
//! содержимого получается та же. Здесь координаты считаются абсолютные, а не
//! вложенные, поэтому ветка одна.
//!
//! Начальная матрица — единичная: `typst_svg::svg` заводит сдвиг страницы
//! только под вылет за обрез (`page_bleed`), а `SvgOptions::default()`
//! оставляет `render_bleed: false`. Тот же самый `SvgOptions::default()`
//! стоит в [`super::pages`], и разойтись они не имеют права — иначе слой
//! уедет ровно на вылет.

use serde::{Deserialize, Serialize};
use typst::layout::{Frame, FrameItem, Transform};
use typst::text::TextItem;

/// Один прогон текста, приведённый к системе координат страницы.
///
/// Координаты в пунктах, `y` растёт вниз — как в SVG страницы. UI пересчитает
/// их в пиксели тем же множителем, который применён к картинке.
///
/// **`y_pt` — базовая линия, а не верх строки.** Так текст ставит сам
/// `typst_svg` (`render_text` рисует глифы от начала координат прогона,
/// переворачивая ось: шрифты живут в системе Y-up). Слой обязан считать так
/// же, иначе строка сядет выше или ниже глифов на величину подъёма шрифта.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TextRun {
    pub text: String,
    pub x_pt: f64,
    /// Базовая линия прогона — см. примечание к типу.
    pub y_pt: f64,
    pub width_pt: f64,
    pub size_pt: f64,
    /// Язык прогона для программы чтения с экрана (FR-032). `None` только
    /// если вёрстка его не назвала — сам Typst всегда называет.
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string | undefined"))]
    pub lang: Option<String>,
}

/// Собирает текстовый слой страницы.
///
/// Порядок прогонов — порядок чтения (SC-011): `Frame::items()` отдаёт
/// элементы в том порядке, в каком вёрстка их поставила, а обход в глубину
/// этот порядок сохраняет. На многоколоночной вёрстке колонки — отдельные
/// группы, идущие в порядке чтения, поэтому отдельного правила для них не
/// нужно; проверяется это тестом, а не рассуждением.
pub fn text_runs(frame: &Frame) -> Vec<TextRun> {
    let mut runs = Vec::new();
    collect(frame, Transform::identity(), &mut runs);
    runs
}

fn collect(frame: &Frame, outer: Transform, runs: &mut Vec<TextRun>) {
    for (pos, item) in frame.items() {
        let placed = outer.pre_concat(Transform::translate(pos.x, pos.y));
        match item {
            FrameItem::Group(group) => {
                collect(&group.frame, placed.pre_concat(group.transform), runs);
            }
            FrameItem::Text(text) => runs.push(run_of(text, placed)),
            // Фигуры, изображения, ссылки и метки текста не несут.
            FrameItem::Shape(..)
            | FrameItem::Image(..)
            | FrameItem::Link(..)
            | FrameItem::Tag(..) => {}
        }
    }
}

fn run_of(text: &TextItem, placed: Transform) -> TextRun {
    let lang = text.lang.as_str();
    TextRun {
        text: text.text.to_string(),
        x_pt: placed.tx.to_pt(),
        y_pt: placed.ty.to_pt(),
        // Ширина берётся у самого `TextItem`: он суммирует `x_advance` глифов
        // с приведением к кеглю (`typst-library-0.15.1/src/text/item.rs`).
        // Своя сумма здесь была бы второй копией того же правила и разошлась
        // бы с вёрсткой на лигатурах.
        width_pt: text.width().to_pt(),
        size_pt: text.size.to_pt(),
        lang: (!lang.is_empty()).then(|| lang.to_owned()),
    }
}
