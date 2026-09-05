//! Журналы приложения (FR-040, FR-041, FR-043, research.md §12).
//!
//! Два ограничения здесь — не пожелания, а проверяемые гарантии:
//!
//! 1. **Содержимое заметок и досок в журнал не попадает никогда** (FR-041,
//!    принцип IX). Это обеспечено формой типа: запись журнала — перечислимое
//!    событие с фиксированным набором полей, а не свободная строка. Записать
//!    туда текст заметки попросту нечем.
//! 2. **Абсолютных путей нет** — только пути относительно корня vault
//!    (research.md §12), чтобы журнал, отправленный пользователем в поддержку
//!    вручную, не раскрывал имя пользователя и структуру его диска.

use std::path::{Path, PathBuf};

/// Суммарный бюджет журналов (FR-043): два файла по 1 МиБ.
pub const MAX_LOG_FILE_BYTES: usize = 1024 * 1024;
pub const LOG_FILE_COUNT: usize = 2;
pub const MAX_TOTAL_LOG_BYTES: usize = MAX_LOG_FILE_BYTES * LOG_FILE_COUNT;

/// Что именно произошло. Перечисление, а не строка: набор событий закрыт, и
/// добавить в журнал произвольный текст невозможно по построению.
///
/// Тип намеренно **не** сериализуемый через serde: поля `&'static str`
/// удерживают набор значений закрытым, а журнал существует только как текст на
/// диске — обратно в типизированный вид он не читается.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    SettingsLoaded,
    SettingsChanged { field: &'static str },
    SettingsUnreadable,
    InvalidFileSetAside,
    StorageBecameReadOnly,
    VaultAdded,
    VaultSwitched,
    VaultForgotten,
    VaultUnavailable,
    EditorModeChanged { mode: &'static str },
    EditorConfigApplied,
    EditorConfigUnreadable,
    ResetToDefaults,
}

impl LogEvent {
    /// Стабильное имя события в журнале. Записывается вручную, а не через
    /// сериализатор: тянуть `serde_json` ради одной строки незачем, а имя
    /// должно быть стабильным независимо от того, как тип сериализуется на
    /// границе WASM/JS.
    pub fn label(&self) -> &'static str {
        match self {
            LogEvent::SettingsLoaded => "SettingsLoaded",
            LogEvent::SettingsChanged { .. } => "SettingsChanged",
            LogEvent::SettingsUnreadable => "SettingsUnreadable",
            LogEvent::InvalidFileSetAside => "InvalidFileSetAside",
            LogEvent::StorageBecameReadOnly => "StorageBecameReadOnly",
            LogEvent::VaultAdded => "VaultAdded",
            LogEvent::VaultSwitched => "VaultSwitched",
            LogEvent::VaultForgotten => "VaultForgotten",
            LogEvent::VaultUnavailable => "VaultUnavailable",
            LogEvent::EditorModeChanged { .. } => "EditorModeChanged",
            LogEvent::EditorConfigApplied => "EditorConfigApplied",
            LogEvent::EditorConfigUnreadable => "EditorConfigUnreadable",
            LogEvent::ResetToDefaults => "ResetToDefaults",
        }
    }

    /// Уточняющее значение события, если оно есть. Всегда из закрытого набора
    /// `&'static str` — произвольный текст сюда не попадает по построению.
    pub fn detail(&self) -> Option<&'static str> {
        match self {
            LogEvent::SettingsChanged { field } => Some(field),
            LogEvent::EditorModeChanged { mode } => Some(mode),
            _ => None,
        }
    }
}

/// Одна запись журнала.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub utc_millis: i64,
    pub event: LogEvent,
    /// Путь **относительно корня vault**, если событие вообще связано с
    /// заметкой. Приводится [`relative_to_vault`]; абсолютный путь сюда попасть
    /// не может.
    pub note_path: Option<PathBuf>,
}

impl LogEntry {
    pub fn new(utc_millis: i64, event: LogEvent) -> Self {
        Self {
            utc_millis,
            event,
            note_path: None,
        }
    }

    /// Прикладывает путь заметки, предварительно сделав его относительным.
    /// Если путь лежит вне vault, он не прикладывается вовсе — лучше запись без
    /// пути, чем запись с абсолютным.
    pub fn with_note(mut self, vault_root: &Path, note_path: &Path) -> Self {
        self.note_path = relative_to_vault(vault_root, note_path);
        self
    }

    /// Сериализация одной строки журнала.
    pub fn render(&self) -> String {
        let mut line = format!("{} {}", self.utc_millis, self.event.label());
        if let Some(detail) = self.event.detail() {
            line.push(' ');
            line.push_str(detail);
        }
        if let Some(path) = &self.note_path {
            line.push(' ');
            line.push_str(&path.to_string_lossy().replace('\\', "/"));
        }
        line
    }
}

/// Приводит путь заметки к корню vault (research.md §12). `None`, если путь
/// вне vault — тогда в журнал не попадает ничего.
pub fn relative_to_vault(vault_root: &Path, note_path: &Path) -> Option<PathBuf> {
    if note_path.is_relative() {
        return Some(note_path.to_path_buf());
    }
    note_path.strip_prefix(vault_root).ok().map(Path::to_path_buf)
}

/// Журнал с ротацией двух файлов (FR-043).
///
/// Ротация «текущий → предыдущий, текущий очищается» вместо усечения по
/// байтам: усечение резало бы строку посередине, а журнал должен оставаться
/// разбираемым построчно.
#[derive(Debug, Default)]
pub struct LogRotation {
    current: String,
    previous: String,
}

impl LogRotation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, entry: &LogEntry) {
        let line = entry.render();
        if self.current.len() + line.len() + 1 > MAX_LOG_FILE_BYTES {
            self.previous = std::mem::take(&mut self.current);
        }
        self.current.push_str(&line);
        self.current.push('\n');
    }

    pub fn current(&self) -> &str {
        &self.current
    }

    pub fn previous(&self) -> &str {
        &self.previous
    }

    pub fn total_bytes(&self) -> usize {
        self.current.len() + self.previous.len()
    }

    /// Всё содержимое журналов — для тест-сканера (research.md §12) и для
    /// показа пользователю (FR-040).
    pub fn all_text(&self) -> String {
        format!("{}{}", self.previous, self.current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_note_paths_are_reduced_to_vault_relative() {
        let relative = relative_to_vault(
            Path::new("/home/user/vault"),
            Path::new("/home/user/vault/sub/note.typ"),
        );
        assert_eq!(relative, Some(PathBuf::from("sub/note.typ")));
    }

    #[test]
    fn path_outside_vault_is_dropped_entirely() {
        // Лучше запись без пути, чем запись с абсолютным путём.
        assert_eq!(
            relative_to_vault(Path::new("/home/user/vault"), Path::new("/etc/passwd")),
            None
        );
    }

    #[test]
    fn rendered_entry_has_no_absolute_paths_and_no_note_text() {
        let entry = LogEntry::new(1700000000000, LogEvent::VaultSwitched).with_note(
            Path::new("/home/user/vault"),
            Path::new("/home/user/vault/секретная.typ"),
        );
        let line = entry.render();
        assert!(!line.contains("/home/user"));
        assert!(line.contains("секретная.typ"));
        assert!(line.contains("VaultSwitched"));
    }

    #[test]
    fn rotation_keeps_total_within_budget() {
        // FR-043: суммарный объём ≤2 МиБ при любом числе записей.
        let mut log = LogRotation::new();
        for i in 0..200_000 {
            log.append(&LogEntry::new(i, LogEvent::SettingsLoaded));
        }
        assert!(
            log.total_bytes() <= MAX_TOTAL_LOG_BYTES,
            "суммарный объём {} превысил бюджет {MAX_TOTAL_LOG_BYTES}",
            log.total_bytes()
        );
        assert!(log.current().len() <= MAX_LOG_FILE_BYTES);
        assert!(log.previous().len() <= MAX_LOG_FILE_BYTES);
    }

    #[test]
    fn rotation_moves_current_to_previous_without_cutting_lines() {
        let mut log = LogRotation::new();
        for i in 0..100_000 {
            log.append(&LogEntry::new(i, LogEvent::SettingsLoaded));
        }
        for text in [log.current(), log.previous()] {
            if text.is_empty() {
                continue;
            }
            assert!(text.ends_with('\n'), "журнал не должен обрываться посередине строки");
        }
    }
}
