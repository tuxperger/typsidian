//! Страница свёрстанного документа (data-model.md «RenderedPage»).
//!
//! Вывод постраничный (`typst_svg::svg(page, ..)`), а не одним полотном
//! `svg_merged`: FR-003 требует перехода к любой странице и различимых границ,
//! а `digest` каждой страницы позволяет UI не перерисовывать то, что не
//! изменилось (research.md §5).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use typst_layout::{Page, PagedDocument};

/// Одна страница документа, готовая к показу.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RenderedPage {
    /// Физический порядковый номер, 0-based.
    pub index: usize,
    /// Логический номер: им управляет `counter(page)`, и он может не совпадать
    /// с физическим.
    pub number: u64,
    pub width_pt: f64,
    pub height_pt: f64,
    /// Визуальный слой.
    ///
    /// Внимание: глифы здесь — контуры `<path>`, а не текст
    /// (`typst-svg-0.15.1/src/text.rs`). Выделение и доступность обеспечивает
    /// отдельный текстовый слой (contracts/text-layer.md), которого в этом
    /// срезе ещё нет.
    pub svg: String,
    /// Отпечаток страницы: единица сравнения между сборками.
    pub digest: String,
}

impl RenderedPage {
    fn from_page(index: usize, page: &Page) -> Self {
        let options = typst_svg::SvgOptions::default();
        let size = page.frame.size();
        Self {
            index,
            number: page.number,
            width_pt: size.x.to_pt(),
            height_pt: size.y.to_pt(),
            svg: typst_svg::svg(page, &options),
            digest: digest_of(page),
        }
    }
}

/// Отпечаток страницы.
///
/// Считается от самого `Page` — тип выводит `Hash`
/// (`typst-layout-0.15.1/src/document.rs:79`), поэтому своего обхода
/// содержимого заводить не нужно. `DefaultHasher` берётся с фиксированными
/// ключами, так что в пределах одной сессии сравнение устойчиво; сравнивать
/// отпечатки между таргетами не требуется — они живут внутри одного клиента.
fn digest_of(page: &Page) -> String {
    let mut hasher = DefaultHasher::new();
    page.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Переводит свёрстанный документ в страницы для показа.
pub fn render_pages(document: &PagedDocument) -> Vec<RenderedPage> {
    document
        .pages()
        .iter()
        .enumerate()
        .map(|(index, page)| RenderedPage::from_page(index, page))
        .collect()
}
