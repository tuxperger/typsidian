//! Список известных vault (FR-019) и непрозрачная идентичность записи
//! (research.md §10).
//!
//! **Ключевое обстоятельство фичи.** Запись **не хранит ссылку на папку** — ни
//! путь, ни хендл. Она хранит выданный при добавлении [`VaultId`] и
//! показываемое расположение. Ссылка живёт в реестре таргета
//! ([`super::folder_ref`]): `PathBuf` в `settings.toml` на native,
//! `FileSystemDirectoryHandle` в IndexedDB на web — сериализовать хендл в TOML
//! невозможно.
//!
//! Причина именно такой формы: в браузере абсолютного пути не существует,
//! `showDirectoryPicker()` отдаёт только имя папки. Идентичность по пути
//! схлопнула бы там две разные папки `notes` в одну запись и лишила бы
//! пользователя доступа к одному из его vault. Поэтому **ни `location`, ни
//! `label` не сравниваются** при установлении тождества — это делает платформа.

use serde::{Deserialize, Serialize};

/// Стабильный непрозрачный идентификатор записи. Не выводится ни из пути, ни из
/// имени папки: платформы, где пути нет, обязаны получать такой же годный
/// идентификатор.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(transparent)]
pub struct VaultId(String);

impl VaultId {
    /// Восстанавливает идентификатор из уже сохранённого значения (чтение
    /// файла настроек, ключ реестра). Для выдачи нового — [`VaultId::mint`].
    pub fn from_raw(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Выдаёт новый идентификатор.
    ///
    /// Источник уникальности — счётчик уже существующих записей плюс хэш
    /// показываемого расположения и метки времени. Случайности намеренно нет:
    /// `getrandom` под `wasm32-unknown-unknown` требует отдельной настройки
    /// источника энтропии, а гарантия нужна слабее криптографической —
    /// достаточно не столкнуться в пределах одного списка, что проверяется
    /// вызывающей стороной по `existing`.
    pub fn mint(existing: &[VaultId], seed: &str, now_millis: i64) -> Self {
        let mut counter = existing.len() as u64;
        loop {
            let digest = blake3::hash(format!("{seed}\u{0}{now_millis}\u{0}{counter}").as_bytes());
            let candidate = Self(digest.to_hex()[..16].to_string());
            if !existing.contains(&candidate) {
                return candidate;
            }
            counter += 1;
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Доступна ли папка записи прямо сейчас (FR-023). Вычисляется при показе
/// списка и в файле настроек не хранится: состояние диска не является
/// настройкой.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    #[default]
    Available,
    Unavailable,
}

/// Известный приложению vault (data-model.md «VaultRecord»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "snake_case")]
pub struct VaultRecord {
    /// Ключ записи. Не зависит от платформы.
    pub id: VaultId,
    /// Показываемое расположение: полный путь (native) либо имя папки (web).
    /// **Не является признаком тождества** (FR-018).
    pub location: String,
    /// Подпись пользователя; показывается вместо `location`, если задана. Не
    /// обязана быть уникальной, в тождестве не участвует (research.md §14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Вычисляется при показе списка, в файле не хранится (FR-023).
    ///
    /// В TOML не попадает не через `skip`, а по построению: запись файла
    /// собирается вручную в [`super::format`] и кладёт только `id`, `location`
    /// и `label`. `skip` здесь был бы вреден — он выключил бы поле и на границе
    /// WASM/JS, где оно как раз нужно, чтобы пометить недоступные записи.
    #[serde(default)]
    pub availability: Availability,
}

impl VaultRecord {
    pub fn new(id: VaultId, location: String) -> Self {
        Self {
            id,
            location,
            label: None,
            availability: Availability::Available,
        }
    }

    /// Что показать пользователю в списке: подпись, если задана, иначе
    /// расположение (FR-018).
    pub fn display_name(&self) -> &str {
        match &self.label {
            Some(label) if !label.trim().is_empty() => label,
            _ => &self.location,
        }
    }
}

/// Ведение списка известных vault (FR-019). Все функции работают по
/// [`VaultId`]: сравнение папок сюда не попадает вообще — оно принадлежит
/// реестру таргета (research.md §10).
pub fn find(records: &[VaultRecord], id: &VaultId) -> Option<usize> {
    records.iter().position(|r| &r.id == id)
}

/// Добавляет запись, сохраняя порядок. Повторное добавление уже известного
/// `id` не создаёт вторую запись, а обновляет показываемое расположение:
/// решение «та же ли это папка» уже принято платформой до вызова.
pub fn upsert(records: &mut Vec<VaultRecord>, record: VaultRecord) {
    match find(records, &record.id) {
        Some(index) => {
            let existing = &mut records[index];
            existing.location = record.location;
            // Подпись, заданная пользователем, повторным добавлением не
            // затирается: она принадлежит ему, а не операции добавления.
            if existing.label.is_none() {
                existing.label = record.label;
            }
        }
        None => records.push(record),
    }
}

/// Задаёт или снимает подпись (FR-018). Возвращает `false`, если записи нет.
pub fn set_label(records: &mut [VaultRecord], id: &VaultId, label: Option<String>) -> bool {
    match find(records, id) {
        Some(index) => {
            // Пустая строка — это снятие подписи, а не подпись из пробелов:
            // иначе в списке появилась бы визуально безымянная запись.
            records[index].label = label.filter(|value| !value.trim().is_empty());
            true
        }
        None => false,
    }
}

/// Убирает запись из списка (FR-024). Папку на диске не трогает — удаление
/// ссылки из реестра таргета выполняет вызывающая сторона.
pub fn forget(records: &mut Vec<VaultRecord>, id: &VaultId) -> bool {
    match find(records, id) {
        Some(index) => {
            records.remove(index);
            true
        }
        None => false,
    }
}

/// Проставляет вычисленную доступность записи (FR-023).
pub fn set_availability(records: &mut [VaultRecord], id: &VaultId, availability: Availability) {
    if let Some(index) = find(records, id) {
        records[index].availability = availability;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, location: &str) -> VaultRecord {
        VaultRecord::new(VaultId::from_raw(id), location.to_string())
    }

    #[test]
    fn minted_ids_are_distinct_for_same_location_and_time() {
        // Две разные папки могут иметь одинаковое имя и быть добавлены в одну
        // миллисекунду — идентификаторы всё равно обязаны различаться
        // (US2 AS9).
        let first = VaultId::mint(&[], "notes", 1000);
        let second = VaultId::mint(std::slice::from_ref(&first), "notes", 1000);
        assert_ne!(first, second);
    }

    #[test]
    fn upsert_reuses_existing_record_instead_of_duplicating() {
        let mut records = vec![record("a", "/work/notes")];
        upsert(&mut records, record("a", "/work/notes"));
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn distinct_ids_with_identical_location_stay_separate() {
        // Ключевая регрессия research.md §10: одинаковое показываемое
        // расположение не схлопывает записи.
        let mut records = vec![record("a", "notes")];
        upsert(&mut records, record("b", "notes"));
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn upsert_does_not_overwrite_user_label() {
        let mut records = vec![record("a", "/work/notes")];
        set_label(&mut records, &VaultId::from_raw("a"), Some("Работа".into()));
        upsert(&mut records, record("a", "/work/notes-moved"));
        assert_eq!(records[0].label.as_deref(), Some("Работа"));
        assert_eq!(records[0].location, "/work/notes-moved");
    }

    #[test]
    fn label_is_shown_instead_of_location_and_blank_clears_it() {
        let mut records = vec![record("a", "/work/notes")];
        set_label(&mut records, &VaultId::from_raw("a"), Some("Работа".into()));
        assert_eq!(records[0].display_name(), "Работа");
        set_label(&mut records, &VaultId::from_raw("a"), Some("   ".into()));
        assert_eq!(records[0].display_name(), "/work/notes");
    }

    #[test]
    fn forget_removes_only_the_named_record() {
        let mut records = vec![record("a", "/work/notes"), record("b", "/personal/notes")];
        assert!(forget(&mut records, &VaultId::from_raw("a")));
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, VaultId::from_raw("b"));
        assert!(!forget(&mut records, &VaultId::from_raw("a")));
    }
}
