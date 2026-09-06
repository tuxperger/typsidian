//! Набор шрифтов, доступный документу (FR-027).
//!
//! Правило спеки: встроенный набор приложения **плюс** шрифты из папки vault.
//! Шрифты, установленные в системе пользователя, не используются — иначе один
//! и тот же vault выглядел бы по-разному на web и на desktop (spec.md →
//! Clarifications).
//!
//! Сейчас встроенный набор берётся из `typst-assets`, вшитого в модуль. Это
//! временно и стоит 9,6 МБ веса: T029 переносит его в отдельные кэшируемые
//! файлы, а сюда байты будут приходить снаружи — форма [`FontSet::from_bytes`]
//! рассчитана именно на это и меняться не будет.

use typst::foundations::Bytes;
use typst::text::Font;

/// Шрифты сборки.
#[derive(Debug, Clone, Default)]
pub struct FontSet {
    fonts: Vec<Font>,
}

impl FontSet {
    /// Собирает набор из готовых байтов.
    ///
    /// Один файл может содержать несколько начертаний (TTC), поэтому из каждого
    /// извлекается всё, что в нём есть.
    pub fn from_bytes(files: impl IntoIterator<Item = Bytes>) -> Self {
        let fonts = files
            .into_iter()
            .flat_map(|data| (0..).map_while(move |index| Font::new(data.clone(), index)))
            .collect();
        Self { fonts }
    }

    /// Встроенный набор приложения.
    pub fn builtin() -> Self {
        Self::from_bytes(typst_assets::fonts().map(|bytes| Bytes::new(bytes.to_vec())))
    }

    /// Добавляет шрифты пользователя из папки vault (FR-027, FR-029).
    pub fn with_vault_fonts(mut self, files: impl IntoIterator<Item = Bytes>) -> Self {
        self.fonts.extend(Self::from_bytes(files).fonts);
        self
    }

    pub fn into_fonts(self) -> Vec<Font> {
        self.fonts
    }

    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }
}
