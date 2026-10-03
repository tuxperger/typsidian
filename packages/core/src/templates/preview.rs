//! Превью заготовки: порт [`PreviewSource`], состояния и кэш (T016-T018).
//!
//! Порт существует по одной причине, названной в contracts/preview-source.md:
//! FR-011 требует отрисованный вид документа, и решение, чем его строить, не
//! принадлежит галерее. Сегодня в фиче включена единственная реализация —
//! [`ExcerptOnlySource`], которая всегда отдаёт фрагмент исходного текста с
//! причиной [`ExcerptReason::CompilerUnavailable`].
//!
//! # Про компилятор, который в проекте с тех пор появился
//!
//! `research.md` §1 этой фичи писался, когда зависимости `typst` в
//! `packages/core/Cargo.toml` не было вовсе. Фича 005 её принесла. Порт от
//! этого не стал лишним — наоборот, он ровно то место, куда добавляется
//! `TypstPreviewSource`: одна реализация, ноль правок в галерее, модели и
//! контрактах. `tasks.md` держит эту работу за пределами списка (§ «Что
//! намеренно не входит»), поэтому здесь её нет.
//!
//! # Главный запрет
//!
//! [`ExcerptReason::CompilerUnavailable`] и [`ExcerptReason::DoesNotCompile`]
//! **не схлопываются**. Схлопнув их, мы сказали бы пользователю, что не
//! компилируются все его заготовки, — он сделал бы единственный доступный
//! вывод, что сломаны они, а не конвейер превью.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Доступен ли конвейер отрисовки в этой сборке
/// (contracts/preview-source.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum PreviewAvailability {
    /// Компилятор есть: возможен [`PreviewState::Rendered`].
    Available,
    /// Компилятора в сборке нет: любое содержимое даёт фрагмент текста.
    CompilerUnavailable,
}

/// Почему карточка показывает фрагмент исходного текста, а не отрисованный вид.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ExcerptReason {
    /// Компилятор доступен, но содержимое не собирается. Это **штатный
    /// случай**, а не ошибка: фрагмент кода — кусок таблицы, строка формулы —
    /// сам по себе обычно и не должен собираться (FR-013, SC-005).
    DoesNotCompile,
    /// Отрисовывать нечем: в этой сборке конвейер превью выключен.
    CompilerUnavailable,
}

/// Почему превью не построено вовсе (в отличие от построенного текстом).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum PreviewUnavailable {
    /// Файла заготовки нет — его удалили или переименовали снаружи.
    NotFound,
    /// Файл есть, но прочитать его как текст нельзя (FR-017). Перезаписывать
    /// такой файл нельзя тоже.
    Unreadable,
    /// Отказ чтения с диска.
    Io,
}

/// Состояние превью одной заготовки (data-model.md «PreviewState»).
///
/// Три запрета, каждый закрыт тестом:
///
/// 1. `CompilerUnavailable` не схлопывается в `DoesNotCompile`;
/// 2. `Unavailable` не схлопывается в `SourceExcerpt` с пустым фрагментом —
///    нечитаемый файл и некомпилируемое содержимое требуют разных действий;
/// 3. `Pending` — состояние карточки, а не галереи: выбор оно не блокирует
///    (FR-014).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "state")]
pub enum PreviewState {
    /// Отрисованный вид документа (FR-011). Сегодня не порождается ни одной
    /// включённой реализацией порта — см. заголовок модуля.
    Rendered { svg: String },
    /// Фрагмент исходного текста и **названная** причина, почему он, а не
    /// отрисованный вид (FR-012).
    SourceExcerpt {
        reason: ExcerptReason,
        excerpt: String,
    },
    /// Заготовка недоступна: карточка помечена, галерея не прерывается
    /// (FR-017).
    Unavailable { reason: PreviewUnavailable },
    /// Превью ещё не построено; место занято плейсхолдером (FR-014).
    Pending,
}

/// Порт построения превью (contracts/preview-source.md).
///
/// `&self`, а не `&mut self`: построение превью — чистое отображение
/// содержимого в состояние. Кэш живёт снаружи ([`PreviewCache`]) намеренно,
/// чтобы политику кэширования можно было не трогать при замене реализации.
pub trait PreviewSource {
    fn availability(&self) -> PreviewAvailability;
    fn preview(&self, content: &str) -> PreviewState;
}

/// Реализация без компилятора: всегда фрагмент исходного текста.
///
/// Причина всегда [`ExcerptReason::CompilerUnavailable`] и **никогда**
/// `DoesNotCompile`: этот источник о компилируемости содержимого ничего не
/// знает и утверждать о ней ничего не имеет права.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExcerptOnlySource;

impl PreviewSource for ExcerptOnlySource {
    fn availability(&self) -> PreviewAvailability {
        PreviewAvailability::CompilerUnavailable
    }

    fn preview(&self, content: &str) -> PreviewState {
        PreviewState::SourceExcerpt {
            reason: ExcerptReason::CompilerUnavailable,
            excerpt: excerpt(content),
        }
    }
}

/// Сколько строк содержимого попадает в карточку.
///
/// Величина от карточки макета «Утилиты» § Шаблоны: там видно шесть строк
/// исходника. Здесь с запасом — пустые строки заготовки тоже считаются.
pub const EXCERPT_LINES: usize = 12;

/// Верхний предел фрагмента в символах.
///
/// Существует ради заготовки без переводов строки: одна строка на мегабайт
/// иначе целиком уехала бы в карточку и в память.
pub const EXCERPT_CHARS: usize = 600;

/// Фрагмент исходного текста для карточки (data-model.md «PreviewState»).
///
/// Режется **по границе строки**: обрубок посреди слова в карточке выглядит
/// повреждением файла, а не сокращением. Предел в символах — второй рубеж, и
/// он тоже не режет посреди символа, потому что считается по `char`, а не по
/// байтам.
pub fn excerpt(content: &str) -> String {
    let mut taken = String::new();

    for (index, line) in content.lines().take(EXCERPT_LINES).enumerate() {
        // Сколько символов ещё поместится, с учётом перевода строки.
        let used = taken.chars().count() + usize::from(index > 0);
        if used >= EXCERPT_CHARS {
            break;
        }
        if index > 0 {
            taken.push('\n');
        }

        let room = EXCERPT_CHARS - used;
        if line.chars().count() <= room {
            taken.push_str(line);
        } else {
            // Длинная строка обрезается по границе слова, если она есть
            // достаточно близко к пределу, — иначе по границе символа.
            taken.push_str(&cut_at_word(line, room));
            break;
        }
    }

    taken
}

/// Обрезает строку до `room` символов, предпочитая границу слова.
///
/// «Достаточно близко» — не меньше трёх четвертей предела: пробел в начале
/// длинной строки границей слова для обрезки не является, обрезав по нему, мы
/// выбросили бы почти весь фрагмент.
fn cut_at_word(line: &str, room: usize) -> String {
    let head: String = line.chars().take(room).collect();
    match head.rfind(char::is_whitespace) {
        Some(space) if head[..space].chars().count() * 4 >= room * 3 => head[..space].to_owned(),
        _ => head,
    }
}

/// Кэш превью по хэшу содержимого (research.md §4).
///
/// Ключ — `BLAKE3` содержимого, а не путь и не время изменения. Из этого
/// следуют оба требования спеки разом: правка заготовки снаружи меняет хэш и
/// превью пересчитывается само (FR-003, SC-010), а переименование файла кэш не
/// инвалидирует.
///
/// Хэш не считается здесь: он уже посчитан индексом заметок и лежит в
/// `FileStamp::content_hash`. Второй `blake3::hash` по тому же содержимому был
/// бы лишним проходом по файлу на каждую карточку.
///
/// Кэш — **производное восстановимое состояние**: авторитетной копией он не
/// является ничему, живёт в памяти и переживать перезапуск не обязан.
#[derive(Debug, Default)]
pub struct PreviewCache {
    entries: HashMap<[u8; 32], PreviewState>,
}

impl PreviewCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Готовое превью для этого содержимого, если оно уже строилось.
    pub fn get(&self, content_hash: &[u8; 32]) -> Option<&PreviewState> {
        self.entries.get(content_hash)
    }

    /// Превью для содержимого: из кэша либо построенное портом.
    ///
    /// Порт вызывается **только при промахе** — это и есть вся политика
    /// кэширования, и она не зависит от того, какая реализация подставлена.
    pub fn preview_with<S: PreviewSource>(
        &mut self,
        source: &S,
        content_hash: [u8; 32],
        content: &str,
    ) -> PreviewState {
        if let Some(known) = self.entries.get(&content_hash) {
            return known.clone();
        }
        let built = source.preview(content);
        self.entries.insert(content_hash, built.clone());
        built
    }

    /// Сколько содержимых уже посчитано. Справочная величина для тестов и
    /// диагностики; решений по ней никто не принимает.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Забывает всё посчитанное. Восстановимо по определению.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_excerpt_stops_at_a_line_boundary() {
        let content = (1..=40)
            .map(|number| format!("строка {number}"))
            .collect::<Vec<_>>()
            .join("\n");
        let taken = excerpt(&content);
        assert_eq!(taken.lines().count(), EXCERPT_LINES);
        assert!(taken.ends_with("строка 12"));
    }

    #[test]
    fn a_single_long_line_is_cut_between_words() {
        let content = "слово ".repeat(400);
        let taken = excerpt(&content);
        assert!(taken.chars().count() <= EXCERPT_CHARS);
        assert!(
            !taken.ends_with("сло"),
            "обрубка посреди слова быть не должно"
        );
    }
}
