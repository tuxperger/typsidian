//! Вид изображения — по содержимому, а не по расширению (FR-006, FR-007,
//! research.md §4).
//!
//! Typst выбирает декодер по расширению пути, поэтому JPEG под именем `.png`
//! не собрался бы. Вид определяется сигнатурой, а расширение имени потом
//! приводится к нему (`name::kept_name`).

use serde::{Deserialize, Serialize};

/// Поддерживаемые виды изображений (FR-006).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ImageKind {
    Png,
    Jpeg,
    Gif,
    Webp,
    Svg,
}

impl ImageKind {
    /// Каноническое расширение вида — им заменяется чужое.
    pub fn extension(self) -> &'static str {
        match self {
            ImageKind::Png => "png",
            ImageKind::Jpeg => "jpg",
            ImageKind::Gif => "gif",
            ImageKind::Webp => "webp",
            ImageKind::Svg => "svg",
        }
    }

    /// Принадлежит ли расширение виду, без учёта регистра: `photo.JPEG` уже
    /// верно называет JPEG, и переименовывать его незачем.
    pub fn accepts_extension(self, extension: &str) -> bool {
        let accepted: &[&str] = match self {
            ImageKind::Png => &["png"],
            ImageKind::Jpeg => &["jpg", "jpeg"],
            ImageKind::Gif => &["gif"],
            ImageKind::Webp => &["webp"],
            ImageKind::Svg => &["svg"],
        };
        accepted
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
    }
}

/// Сколько начала файла просматривается в поисках `<svg`.
const SVG_PROBE_BYTES: usize = 4096;

/// Вид изображения по первым байтам. `None` — вид не поддерживается: файл не
/// копируется и называется в отчёте (FR-006).
pub fn sniff(bytes: &[u8]) -> Option<ImageKind> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(ImageKind::Png);
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(ImageKind::Jpeg);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(ImageKind::Gif);
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(ImageKind::Webp);
    }
    is_svg(bytes).then_some(ImageKind::Svg)
}

/// Текст, в котором после BOM, пробелов, объявления XML, комментариев и
/// `<!DOCTYPE …>` первым идёт элемент `svg`.
fn is_svg(bytes: &[u8]) -> bool {
    let probe = &bytes[..bytes.len().min(SVG_PROBE_BYTES)];
    // Конец пробы мог разрезать многобайтовый символ: срез до последней целой
    // границы, а бинарное содержимое отсеивается целиком.
    let text = match std::str::from_utf8(probe) {
        Ok(text) => text,
        Err(error) if error.error_len().is_none() => {
            std::str::from_utf8(&probe[..error.valid_up_to()]).unwrap_or_default()
        }
        Err(_) => return false,
    };
    let mut rest = text.strip_prefix('\u{feff}').unwrap_or(text);
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("<?") {
            match after.find("?>") {
                Some(end) => rest = &after[end + 2..],
                None => return false,
            }
        } else if let Some(after) = rest.strip_prefix("<!--") {
            match after.find("-->") {
                Some(end) => rest = &after[end + 3..],
                None => return false,
            }
        } else if rest
            .get(..9)
            .is_some_and(|head| head.eq_ignore_ascii_case("<!doctype"))
        {
            match doctype_end(rest) {
                Some(end) => rest = &rest[end..],
                None => return false,
            }
        } else {
            break;
        }
    }
    let Some(after) = rest.strip_prefix("<svg") else {
        return false;
    };
    // Граница имени: `<svgx` — другой элемент.
    after
        .chars()
        .next()
        .is_none_or(|next| next.is_whitespace() || next == '>' || next == '/')
}

/// Конец объявления `<!DOCTYPE …>` с учётом внутреннего подмножества `[…]`.
fn doctype_end(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (at, character) in text.char_indices() {
        match character {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            '>' if depth == 0 => return Some(at + 1),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_of_the_five_kinds() {
        assert_eq!(
            sniff(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]),
            Some(ImageKind::Png)
        );
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(ImageKind::Jpeg));
        assert_eq!(sniff(b"GIF89a...."), Some(ImageKind::Gif));
        assert_eq!(sniff(b"GIF87a"), Some(ImageKind::Gif));
        assert_eq!(sniff(b"RIFF\x10\0\0\0WEBPVP8 "), Some(ImageKind::Webp));
        assert_eq!(
            sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            Some(ImageKind::Svg)
        );
    }

    #[test]
    fn svg_after_prolog_comments_and_doctype() {
        let text = "\u{feff}  <?xml version=\"1.0\"?>\n<!-- нарисовано -->\n<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" [ <!ENTITY a \"b\"> ]>\n<svg>";
        assert_eq!(sniff(text.as_bytes()), Some(ImageKind::Svg));
    }

    #[test]
    fn not_images() {
        assert_eq!(sniff(b"%PDF-1.7"), None);
        assert_eq!(sniff(b"\0\0\0\x18ftypheic"), None);
        assert_eq!(sniff(b""), None);
        assert_eq!(sniff("просто текст".as_bytes()), None);
        assert_eq!(sniff(b"<svgx>"), None);
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVE"), None);
    }

    #[test]
    fn extension_belongs_to_kind_regardless_of_case() {
        assert!(ImageKind::Jpeg.accepts_extension("JPEG"));
        assert!(ImageKind::Jpeg.accepts_extension("jpg"));
        assert!(!ImageKind::Jpeg.accepts_extension("png"));
        assert_eq!(ImageKind::Jpeg.extension(), "jpg");
    }
}
