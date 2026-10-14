//! Смещения в тексте заметки: байты UTF-8 ядра ↔ кодовые единицы UTF-16 редактора.
//!
//! Разбор Typst считает байтами, CodeMirror — единицами UTF-16. В заметке на
//! кириллице это разные числа с первой же буквы, поэтому наружу ядро отдаёт
//! смещения только в UTF-16 (фичи 008, 024).

/// Смещение в байтах → в кодовых единицах UTF-16.
pub(crate) fn utf16_at(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].encode_utf16().count()
}

/// Смещение в кодовых единицах UTF-16 → в байтах.
///
/// Смещение внутри суррогатной пары или за концом текста прижимается к
/// ближайшей границе символа слева: редактор такого не пришлёт, а паники из-за
/// чужой ошибки на границе быть не должно.
pub(crate) fn byte_at(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (index, ch) in text.char_indices() {
        if units >= utf16 {
            return index;
        }
        units += ch.len_utf16();
        if units > utf16 {
            return index;
        }
    }
    text.len()
}

/// Перевод байтов в UTF-16 для неубывающих запросов — одним проходом по тексту.
///
/// `utf16_at` на каждый фрагмент подсветки дал бы квадрат от длины заметки:
/// фрагментов тысячи, и каждый считал бы текст с начала. Здесь счёт продолжается
/// с прошлого запроса. Запрос левее прошлого не ошибка, а начало заново.
pub(crate) struct Utf16Index<'a> {
    text: &'a str,
    byte: usize,
    units: usize,
}

impl<'a> Utf16Index<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Self {
            text,
            byte: 0,
            units: 0,
        }
    }

    pub(crate) fn at(&mut self, byte: usize) -> usize {
        let byte = byte.min(self.text.len());
        if byte < self.byte {
            self.byte = 0;
            self.units = 0;
        }
        // Байтовые диапазоны разбора всегда на границе символа; если нет —
        // счёт останавливается на последней границе слева, как у `byte_at`.
        for ch in self.text[self.byte..].chars() {
            let next = self.byte + ch.len_utf8();
            if next > byte {
                break;
            }
            self.byte = next;
            self.units += ch.len_utf16();
        }
        self.units
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_round_trip_through_utf16() {
        let text = "Ж𝔸x";
        // Ж — 2 байта, 1 единица; 𝔸 — 4 байта, 2 единицы; x — 1 и 1.
        assert_eq!(utf16_at(text, 2), 1);
        assert_eq!(utf16_at(text, 6), 3);
        assert_eq!(byte_at(text, 1), 2);
        assert_eq!(byte_at(text, 3), 6);
        assert_eq!(byte_at(text, 2), 2, "середина пары прижимается влево");
        assert_eq!(byte_at(text, 99), text.len());
    }

    #[test]
    fn index_agrees_with_utf16_at_for_rising_and_falling_queries() {
        let text = "= Заголовок 𝔸\n#let x = \"ё\"";
        let mut index = Utf16Index::new(text);
        let bytes: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        for &byte in &bytes {
            assert_eq!(index.at(byte), utf16_at(text, byte));
        }
        assert_eq!(index.at(text.len()), utf16_at(text, text.len()));
        // Назад — счёт заново, а не мусор.
        assert_eq!(index.at(2), utf16_at(text, 2));
        assert_eq!(index.at(999), utf16_at(text, text.len()));
    }
}
