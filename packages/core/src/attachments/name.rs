//! Имя вложения (FR-009, FR-012, research.md §5, §8).
//!
//! Буфер и камера: `<имя заметки>-ГГГГ-ММ-ДД-ЧЧММСС.<расширение>`. Перетащенный
//! и выбранный файл сохраняет своё имя после очистки. Vault доставляется на
//! другие устройства (SC-006), поэтому символы, недопустимые на Android или
//! Windows, заменяются; кавычка к тому же попала бы в строку Typst.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::kind::ImageKind;

/// Момент вставки — местное время устройства, собранное в UI: часов в ядре нет
/// ни на одном таргете.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct InsertMoment {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// Имя по умолчанию, если от исходного ничего не осталось.
const FALLBACK_BASE: &str = "image";
/// Сколько суффиксов пробовать, прежде чем сдаться (`-2` … `-10000`).
const MAX_SUFFIX: u32 = 10_000;

/// Имя вложения из буфера или с камеры (обязательство 4). Негодные значения
/// момента прижимаются к границам: имя обязано получиться всегда.
pub fn generated_name(note_path: &Path, moment: &InsertMoment, kind: ImageKind) -> String {
    let stem = note_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut base = clean(&stem);
    if base.is_empty() {
        base = FALLBACK_BASE.to_owned();
    }
    format!(
        "{base}-{:04}-{:02}-{:02}-{:02}{:02}{:02}.{}",
        moment.year.min(9999),
        moment.month.clamp(1, 12),
        moment.day.clamp(1, 31),
        moment.hour.min(23),
        moment.minute.min(59),
        moment.second.min(59),
        kind.extension()
    )
}

/// Имя перетащенного или выбранного файла (обязательство 5): основа
/// сохраняется, расширение меняется, только если не принадлежит виду.
pub fn kept_name(original: &str, kind: ImageKind) -> String {
    let last = original.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned = clean(last);
    let (base, extension) = match cleaned.rfind('.') {
        Some(dot) if dot > 0 => (&cleaned[..dot], Some(&cleaned[dot + 1..])),
        _ => (cleaned.as_str(), None),
    };
    let base = base.trim_end_matches(['.', ' ']);
    let base = if base.is_empty() { FALLBACK_BASE } else { base };
    match extension {
        Some(extension) if kind.accepts_extension(extension) => format!("{base}.{extension}"),
        _ => format!("{base}.{}", kind.extension()),
    }
}

/// Свободное имя (обязательство 6): само имя, если оно не занято, иначе `-2`,
/// `-3`, … перед расширением; `None`, если не нашлось за 9 999 попыток.
pub fn free_name(name: &str, is_taken: impl Fn(&str) -> bool) -> Option<String> {
    if !is_taken(name) {
        return Some(name.to_owned());
    }
    let (base, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    (2..=MAX_SUFFIX)
        .map(|n| format!("{base}-{n}{extension}"))
        .find(|candidate| !is_taken(candidate))
}

/// Символы, недопустимые в имени файла хоть на одной платформе доставки, —
/// на `-`; пробелы и точки по краям срезаются.
fn clean(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            control if control.is_control() => '-',
            other => other,
        })
        .collect();
    replaced.trim_matches([' ', '.']).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOMENT: InsertMoment = InsertMoment {
        year: 2026,
        month: 10,
        day: 10,
        hour: 14,
        minute: 23,
        second: 5,
    };

    #[test]
    fn generated_name_carries_the_note_and_the_moment() {
        assert_eq!(
            generated_name(
                Path::new("Диссертация/Результаты.typ"),
                &MOMENT,
                ImageKind::Png
            ),
            "Результаты-2026-10-10-142305.png"
        );
    }

    #[test]
    fn out_of_range_moment_is_clamped() {
        let odd = InsertMoment {
            month: 13,
            day: 0,
            hour: 25,
            ..MOMENT
        };
        assert_eq!(
            generated_name(Path::new("a.typ"), &odd, ImageKind::Jpeg),
            "a-2026-12-01-232305.jpg"
        );
    }

    #[test]
    fn kept_name_keeps_the_base_and_fixes_the_extension() {
        assert_eq!(kept_name("схема.png", ImageKind::Png), "схема.png");
        assert_eq!(kept_name("Фото.JPEG", ImageKind::Jpeg), "Фото.JPEG");
        assert_eq!(kept_name("photo.png", ImageKind::Jpeg), "photo.jpg");
        assert_eq!(
            kept_name("без расширения", ImageKind::Gif),
            "без расширения.gif"
        );
        assert_eq!(kept_name("C:\\tmp\\a:b?.png", ImageKind::Png), "a-b-.png");
        assert_eq!(kept_name("  . ", ImageKind::Webp), "image.webp");
        assert_eq!(kept_name("", ImageKind::Svg), "image.svg");
    }

    #[test]
    fn free_name_adds_a_suffix_before_the_extension() {
        let taken = ["схема.png", "схема-2.png"];
        assert_eq!(
            free_name("схема.png", |name| taken.contains(&name)),
            Some("схема-3.png".into())
        );
        assert_eq!(free_name("новое.png", |_| false), Some("новое.png".into()));
        assert_eq!(free_name("x.png", |_| true), None);
    }
}
