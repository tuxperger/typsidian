//! Снимок состояния файла на диске — общий примитив ядра.
//!
//! Поднят сюда из `vault::index` (research.md §8 фичи 004): та же семантика
//! «файл действительно изменился» нужна и обнаружению конфликтов vault
//! (FR-021 фичи 001), и признаку устаревания конфига режима редактирования
//! (FR-026 фичи 004). Вторая реализация была бы вторым источником истины
//! (принцип I), поэтому модуль общий, а `vault::index` его ре-экспортирует —
//! существующий код и тесты продолжают ссылаться на прежний путь.

use serde::{Deserialize, Serialize};

/// Дешёвые метаданные (mtime, size) плюс хэш содержимого.
///
/// Хэш — не избыточность: сравнение только по mtime даёт и ложные
/// срабатывания (программы правят время без изменения содержимого), и
/// пропуски (грубое разрешение mtime на части файловых систем). Он же
/// корректно гасит признак расхождения, когда файл изменили и вернули к
/// прежнему содержимому.
///
/// `Tsify` (только под wasm32) порождает TS-тип из этого определения — принцип
/// IV конституции: типы границы WASM/JS не дублируются вручную.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct FileStamp {
    pub mtime_millis: i64,
    pub size: u64,
    #[cfg_attr(target_arch = "wasm32", tsify(type = "number[]"))]
    pub content_hash: [u8; 32],
}

impl FileStamp {
    pub fn from_content(mtime_millis: i64, content: &[u8]) -> Self {
        Self {
            mtime_millis,
            size: content.len() as u64,
            content_hash: *blake3::hash(content).as_bytes(),
        }
    }

    /// Дешёвый предфильтр: стоит ли вообще читать файл, чтобы сравнить
    /// содержимое. Отвечает на «возможно, изменилось», а не на «изменилось».
    pub fn metadata_differs(&self, other: &Self) -> bool {
        self.mtime_millis != other.mtime_millis || self.size != other.size
    }

    /// Содержимое действительно разошлось — единственный признак, по которому
    /// принимаются решения (конфликт, устаревание конфига).
    pub fn content_differs(&self, other: &Self) -> bool {
        self.content_hash != other.content_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touched_file_with_same_content_differs_only_in_metadata() {
        let known = FileStamp::from_content(100, "одно и то же".as_bytes());
        let touched = FileStamp::from_content(900, "одно и то же".as_bytes());
        assert!(known.metadata_differs(&touched));
        assert!(!known.content_differs(&touched));
    }

    #[test]
    fn changed_content_differs_by_hash() {
        let before = FileStamp::from_content(100, "было".as_bytes());
        let after = FileStamp::from_content(100, "стало".as_bytes());
        assert!(before.content_differs(&after));
    }
}
