//! Нормализация содержимого доски и хэш по ней (фича 002, research.md §5).
//!
//! Зачем нормализация, а не хэш файла целиком: движок пишет в каждый элемент
//! счётчики ревизий (`version`, `versionNonce`, `updated`, `seed`), которые
//! меняются от простого открытия доски. Хэш по сырому файлу зажигал бы
//! индикатор устаревания на всех вставках доски, которую пользователь только
//! посмотрел, — критерий SC-008 выполнялся бы формально и при этом бесполезно.
//!
//! Ядро **не воспроизводит модель Excalidraw**: оно вычитает из JSON ровно то,
//! что не является содержимым рисунка, и хэширует остальное как есть
//! (research.md §9). Появление в движке новых полей ломает этот код только
//! если они окажутся волатильными — тогда их добавляют в список ниже.

use serde::{Deserialize, Serialize};

/// Поля, меняющиеся без изменения рисунка. Список закрыт намеренно: всё, что
/// сюда не попало, считается содержимым и влияет на хэш.
const VOLATILE_ELEMENT_FIELDS: &[&str] = &["version", "versionNonce", "updated", "seed"];

/// Хэш нормализованного содержимого доски (data-model.md «BoardDigest»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BoardDigest {
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    value: String,
}

impl BoardDigest {
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Хэш, прочитанный из метаданных снимка (research.md §5).
    ///
    /// `None` на всём, что не является нашим хэшем: файл снимка мог быть
    /// заменён сторонним инструментом, и мусор в атрибуте обязан привести к
    /// «сравнить не удалось» (`EmbedState::Undeterminable`), а не к сравнению с
    /// мусором, которое дало бы `Stale` и предложило обновить снимок без
    /// причины.
    pub fn from_hex(value: &str) -> Option<Self> {
        let expected_len = blake3::OUT_LEN * 2;
        if value.len() != expected_len
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            || value.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return None;
        }
        Some(Self {
            value: value.to_string(),
        })
    }
}

/// Почему хэш не вычислен.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum DigestError {
    /// Файл доски не разбирается как JSON — повреждён или написан
    /// несовместимой версией движка (FR-017). Перезаписывать такой файл нельзя.
    #[error("содержимое доски не разбирается")]
    Unreadable,
}

/// Хэш содержимого доски.
///
/// В хэш входят элементы рисунка в их порядке и вложения (`files`); не входят
/// счётчики ревизий, `appState` целиком — включая положение и масштаб камеры
/// (spec.md Assumptions) — и порядок ключей в объектах JSON.
pub fn digest(content: &[u8]) -> Result<BoardDigest, DigestError> {
    let parsed: serde_json::Value =
        serde_json::from_slice(content).map_err(|_| DigestError::Unreadable)?;

    let mut canonical = serde_json::Map::new();
    canonical.insert("elements".to_string(), normalized_elements(&parsed));
    if let Some(files) = parsed.get("files") {
        canonical.insert("files".to_string(), files.clone());
    }

    // `serde_json::Map` по умолчанию — `BTreeMap`, поэтому сериализация уже
    // канонична по ключам: отдельного упорядочивания не требуется.
    let bytes = serde_json::to_vec(&serde_json::Value::Object(canonical))
        .map_err(|_| DigestError::Unreadable)?;

    Ok(BoardDigest {
        value: blake3::hash(&bytes).to_hex().to_string(),
    })
}

/// Элементы без счётчиков ревизий. Порядок элементов сохраняется: он и есть
/// порядок отрисовки, то есть содержимое.
fn normalized_elements(parsed: &serde_json::Value) -> serde_json::Value {
    let Some(elements) = parsed.get("elements").and_then(|value| value.as_array()) else {
        return serde_json::Value::Array(Vec::new());
    };

    let normalized = elements
        .iter()
        .map(|element| match element {
            serde_json::Value::Object(fields) => {
                let kept = fields
                    .iter()
                    .filter(|(key, _)| !VOLATILE_ELEMENT_FIELDS.contains(&key.as_str()))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<serde_json::Map<_, _>>();
                serde_json::Value::Object(kept)
            }
            other => other.clone(),
        })
        .collect::<Vec<_>>();

    serde_json::Value::Array(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_RECTANGLE: &str = r#"{
        "type": "excalidraw",
        "version": 2,
        "elements": [
            {"id": "a", "type": "rectangle", "x": 1, "y": 2, "version": 7,
             "versionNonce": 111, "updated": 1700000000000, "seed": 42}
        ],
        "appState": {"scrollX": 0, "scrollY": 0, "zoom": {"value": 1}},
        "files": {}
    }"#;

    #[test]
    fn revision_counters_do_not_change_the_digest() {
        // Ключевое свойство: открыл доску, ничего не нарисовал — хэш прежний.
        let touched = ONE_RECTANGLE
            .replace("\"version\": 7", "\"version\": 9")
            .replace("\"versionNonce\": 111", "\"versionNonce\": 999")
            .replace("\"updated\": 1700000000000", "\"updated\": 1700000009999")
            .replace("\"seed\": 42", "\"seed\": 4242");

        assert_eq!(
            digest(ONE_RECTANGLE.as_bytes()).unwrap(),
            digest(touched.as_bytes()).unwrap()
        );
    }

    #[test]
    fn camera_position_is_not_content() {
        // spec.md Assumptions: положение и масштаб камеры содержимым не
        // считаются и между сессиями не гарантируются.
        let panned = ONE_RECTANGLE.replace("\"scrollX\": 0", "\"scrollX\": 512");
        assert_eq!(
            digest(ONE_RECTANGLE.as_bytes()).unwrap(),
            digest(panned.as_bytes()).unwrap()
        );
    }

    #[test]
    fn geometry_change_does_change_the_digest() {
        let moved = ONE_RECTANGLE.replace("\"x\": 1", "\"x\": 300");
        assert_ne!(
            digest(ONE_RECTANGLE.as_bytes()).unwrap(),
            digest(moved.as_bytes()).unwrap()
        );
    }

    #[test]
    fn element_order_is_content() {
        let two = r#"{"elements":[{"id":"a"},{"id":"b"}]}"#;
        let swapped = r#"{"elements":[{"id":"b"},{"id":"a"}]}"#;
        assert_ne!(
            digest(two.as_bytes()).unwrap(),
            digest(swapped.as_bytes()).unwrap()
        );
    }

    #[test]
    fn key_order_in_json_is_not_content() {
        let one = r#"{"elements":[{"id":"a","type":"rectangle"}]}"#;
        let other = r#"{"elements":[{"type":"rectangle","id":"a"}]}"#;
        assert_eq!(
            digest(one.as_bytes()).unwrap(),
            digest(other.as_bytes()).unwrap()
        );
    }

    #[test]
    fn broken_content_is_reported_and_not_hashed() {
        assert_eq!(digest(b"{not json"), Err(DigestError::Unreadable));
    }

    #[test]
    fn empty_board_has_a_digest() {
        // Пустая доска — штатный случай: её можно вставить в заметку (US2 AS3).
        assert!(digest(br#"{"type":"excalidraw","elements":[]}"#).is_ok());
    }
}
