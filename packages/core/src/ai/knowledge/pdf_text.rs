//! Текст PDF по страницам для импорта заметок в граф знаний (фича 055, research.md §3).
//! Страницы прогоняются через интерпретатор `hayro-interpret` (чистый Rust, оба таргета, он
//! же рисует PDF-картинки в Typst); из каждого глифа берётся Unicode (`ToUnicode`, имя
//! глифа). Порядок — как в потоке содержимого страницы, перенос строки — по смене
//! вертикального положения, пробел — по зазору между глифами.
//!
//! PDF приходит из vault, то есть может быть враждебным: размер проверяется до разбора,
//! число страниц и объём текста ограничены. PDF без единого глифа с Unicode — «нет текста»
//! (скан, картинка): ядро не угадывает текст по картинке.

use hayro_interpret::font::Glyph;
use hayro_interpret::hayro_cmap::BfString;
use hayro_interpret::hayro_syntax::Pdf;
use hayro_interpret::{
    interpret_page, BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask,
};
use kurbo::{Affine, BezPath, Point, Rect};

/// Предел размера PDF, байт: проверяется по метаданным до чтения.
pub const MAX_PDF_BYTES: u64 = 10 * 1024 * 1024;
/// Страниц, которые разбираются; больше — файл отклоняется целиком.
pub const MAX_PDF_PAGES: usize = 300;
/// Символов текста одного PDF; больше — файл отклоняется целиком.
pub const MAX_PDF_CHARS: usize = 1_000_000;

/// Почему PDF не разобран.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfError {
    /// Больше [`MAX_PDF_BYTES`], [`MAX_PDF_PAGES`] или [`MAX_PDF_CHARS`].
    TooLarge,
    /// Не PDF, повреждён или зашифрован.
    Unreadable,
}

/// Текст страниц; у страницы без текста строка пустая.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfText {
    pub pages: Vec<String>,
}

impl PdfText {
    /// Есть ли в PDF хоть какой-то текст.
    pub fn has_text(&self) -> bool {
        self.pages.iter().any(|page| !page.trim().is_empty())
    }
}

/// Извлекает текст страниц.
///
/// Чужой разборщик на враждебном файле может паниковать: под native паника
/// перехватывается и файл пропускается как нечитаемый, а не роняет всю команду
/// (ревью 055, M1). Под wasm32 с `panic=abort` перехвата нет — остаточный риск записан
/// в спеке 069.
pub fn pdf_text(bytes: &[u8]) -> Result<PdfText, PdfError> {
    if bytes.len() as u64 > MAX_PDF_BYTES {
        return Err(PdfError::TooLarge);
    }
    guarded(|| extract(bytes))
}

#[cfg(not(target_arch = "wasm32"))]
fn guarded(run: impl FnOnce() -> Result<PdfText, PdfError>) -> Result<PdfText, PdfError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).unwrap_or(Err(PdfError::Unreadable))
}

#[cfg(target_arch = "wasm32")]
fn guarded(run: impl FnOnce() -> Result<PdfText, PdfError>) -> Result<PdfText, PdfError> {
    run()
}

fn extract(bytes: &[u8]) -> Result<PdfText, PdfError> {
    let pdf = Pdf::new(bytes.to_vec()).map_err(|_| PdfError::Unreadable)?;
    let pages = pdf.pages();
    if pages.len() > MAX_PDF_PAGES {
        return Err(PdfError::TooLarge);
    }
    let cache = InterpreterCache::new();
    let mut out = Vec::with_capacity(pages.len());
    let mut total = 0usize;
    for page in pages.iter() {
        let mut extractor = Extractor::default();
        let mut context = Context::new(
            Affine::IDENTITY,
            Rect::new(0.0, 0.0, 1.0, 1.0),
            &cache,
            pdf.xref(),
            InterpreterSettings::default(),
        );
        interpret_page(page, &mut context, &mut extractor);
        let text = extractor.finish();
        total = total.saturating_add(text.chars().count());
        if total > MAX_PDF_CHARS {
            return Err(PdfError::TooLarge);
        }
        out.push(text);
    }
    Ok(PdfText { pages: out })
}

/// Сколько символов страницы собираем, прежде чем перестать: страховка от страницы-бомбы.
const PAGE_CHARS_CAP: usize = MAX_PDF_CHARS + 1;

#[derive(Default)]
struct Extractor {
    text: String,
    chars: usize,
    /// Положение, кегль и ожидаемый шаг предыдущего глифа.
    last: Option<(Point, f64, f64)>,
}

impl Extractor {
    fn finish(self) -> String {
        let mut out = String::with_capacity(self.text.len());
        let mut blank = 0;
        for line in self.text.lines() {
            let line = line.trim();
            if line.is_empty() {
                blank += 1;
                continue;
            }
            if !out.is_empty() {
                out.push('\n');
                if blank > 0 {
                    out.push('\n');
                }
            }
            blank = 0;
            out.push_str(line);
        }
        out
    }
}

impl Device<'_> for Extractor {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}

    fn set_blend_mode(&mut self, _: BlendMode) {}

    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'_>, _: &PathDrawMode) {}

    fn push_clip_path(&mut self, _: &ClipPath) {}

    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}

    fn draw_glyph(
        &mut self,
        glyph: &Glyph<'_>,
        transform: Affine,
        glyph_transform: Affine,
        _: &Paint<'_>,
        _: &GlyphDrawMode,
    ) {
        if self.chars >= PAGE_CHARS_CAP {
            return;
        }
        let Some(unicode) = glyph.as_unicode() else {
            return;
        };
        let combined = transform * glyph_transform;
        let at = combined * Point::new(0.0, 0.0);
        let [a, b, c, d, _, _] = combined.as_coeffs();
        // Контур глифа задан в тысячных em, поэтому кегль — масштаб на 1000.
        let size = c.hypot(d) * 1000.0;
        let step = match glyph {
            Glyph::Outline(outline) => outline
                .advance_width()
                .map(|width| f64::from(width) * a.hypot(b)),
            Glyph::Type3(_) => None,
        };
        if let Some((previous, previous_size, expected)) = self.last {
            let scale = previous_size.max(size);
            let dy = (at.y - previous.y).abs();
            let dx = at.x - previous.x;
            if dy > scale * 0.5 {
                self.text.push('\n');
            } else if dx - expected > scale * 0.25 || dx < -scale {
                // Зазор шире шага предыдущего глифа (или возврат назад на той же строке).
                self.text.push(' ');
            }
        }
        match unicode {
            BfString::Char(ch) => {
                self.text.push(ch);
                self.chars += 1;
            }
            BfString::String(s) => {
                self.chars += s.chars().count();
                self.text.push_str(&s);
            }
        }
        self.last = Some((at, size, step.unwrap_or(size * 0.5)));
    }

    fn draw_image(&mut self, _: Image<'_, '_>, _: Affine) {}

    fn pop_clip_path(&mut self) {}

    fn pop_transparency_group(&mut self) {}
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// Паника разборщика не выходит наружу: файл становится нечитаемым (ревью 055, M1).
    #[test]
    fn a_panic_in_the_parser_skips_the_file() {
        let result = guarded(|| panic!("разборщик упал"));
        assert_eq!(result.unwrap_err(), PdfError::Unreadable);
        let fine = guarded(|| Ok(PdfText { pages: Vec::new() }));
        assert!(fine.is_ok());
    }
}
