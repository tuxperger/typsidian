//! Режим редактирования: наблюдаемое состояние и шов к будущему мосту
//! (contracts/editor-mode-bridge.md, research.md §7).
//!
//! **Моста к Neovim в кодовой базе не существует.** Этот модуль описывает
//! только наблюдаемые состояния и переходы — протокол общения с внешним
//! процессом принадлежит будущей фиче со своим `/speckit-clarify` (принцип V).
//! Появление реального моста добавит вторую реализацию [`EditorModeBridge`]
//! рядом с [`NullBridge`], не меняя публичного API настроек.

use crate::stamp::FileStamp;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Почему полноценный режим недоступен или отказал (FR-034, FR-035).
/// Перечисление, а не строка: UI сопоставляет вариантам формулировки, но не
/// придумывает новых состояний.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum OutageReason {
    /// Платформа не может порождать внешние процессы — постоянное состояние
    /// веб-клиента (research.md §7).
    ExternalProcessesUnavailable,
    /// Подсистема есть, но на этом устройстве не установлена/не найдена.
    BackendNotInstalled,
    /// Процесс завершился или перестал отвечать уже во время работы.
    BackendStopped,
    /// Путь к конфигу задан, но на **этом** устройстве его нет (фича 006,
    /// FR-027).
    ///
    /// Появился вместе с переездом настроек внутрь vault: путь вроде
    /// `/home/apokryf/.config/nvim` едет с папкой заметок на другую машину и
    /// там не существует. Отдельная причина, а не `BackendNotInstalled`:
    /// подсистема на месте, не найден именно конфиг, и пользователю нужно
    /// поправить путь, а не устанавливать Neovim.
    ///
    /// Третье состояние режима при этом **не** заводится: действует уже
    /// установленное фичей 004 правило — `PlainInput` с названной причиной,
    /// редактирование остаётся доступным, автоматического отката на упрощённую
    /// эмуляцию не происходит.
    ConfigMissingOnThisDevice,
}

/// Конфиг полноценного режима (data-model.md «EditorConfig»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct EditorConfig {
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub path: PathBuf,
    /// Снимок файла в момент применения; `None` — конфиг не применялся.
    pub applied_stamp: Option<FileStamp>,
    /// Вычисляется, в файле настроек не хранится.
    pub disk_state: ConfigDiskState,
}

impl EditorConfig {
    pub fn unapplied(path: PathBuf) -> Self {
        Self {
            path,
            applied_stamp: None,
            disk_state: ConfigDiskState::Missing,
        }
    }
}

/// Состояние файла конфига относительно применённого (FR-026).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum ConfigDiskState {
    InSync,
    /// «Изменён, не применён». Применение — только явным действием
    /// пользователя, автоматического перечитывания нет (FR-026).
    Changed,
    Missing,
    Unreadable,
}

/// Вычисляет состояние файла конфига по применённому снимку и текущему.
///
/// Сравнение идёт по хэшу содержимого (research.md §8), поэтому изменение
/// файла с последующим возвратом прежнего содержимого корректно даёт
/// [`ConfigDiskState::InSync`], а не ложное `Changed`.
pub fn disk_state(applied: Option<&FileStamp>, current: Option<&FileStamp>) -> ConfigDiskState {
    match (applied, current) {
        (_, None) => ConfigDiskState::Missing,
        // Конфиг ещё не применялся: файл есть, но он не «изменён относительно
        // применённого» — применённого не существует.
        (None, Some(_)) => ConfigDiskState::Changed,
        (Some(applied), Some(current)) => {
            if applied.content_differs(current) {
                ConfigDiskState::Changed
            } else {
                ConfigDiskState::InSync
            }
        }
    }
}

/// Текущее наблюдаемое состояние редактирования — **три варианта**
/// (data-model.md «EditorMode», кларификация 2026-08-09).
///
/// Трёхвариантный union вместо булева флага сделан намеренно: пропуск
/// `PlainInput` в разборе на стороне UI обязан быть ошибкой компиляции
/// (FR-025, принцип IV).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "state")]
pub enum EditorMode {
    FullBackend {
        config: EditorConfig,
    },
    SimplifiedEmulation,
    /// Наступает **сам** при отказе внешнего процесса и покидается **только**
    /// явным действием пользователя (FR-035).
    PlainInput {
        reason: OutageReason,
    },
}

impl EditorMode {
    pub fn is_full_backend(&self) -> bool {
        matches!(self, EditorMode::FullBackend { .. })
    }
}

/// Доступен ли полноценный режим на этом устройстве (FR-034).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum BridgeAvailability {
    Available,
    Unavailable { reason: OutageReason },
}

/// Отказ операции над мостом.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum BridgeError {
    #[error("полноценный режим недоступен на этом устройстве")]
    Unavailable { reason: OutageReason },
    #[error("файл конфига не найден по указанному пути")]
    ConfigMissing,
    #[error("файл конфига не читается")]
    ConfigUnreadable,
}

/// События изменения состояния моста.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "type")]
pub enum BridgeEvent {
    EditorModeChanged { mode: EditorMode },
}

/// Шов к подсистеме полноценного режима (contracts/editor-mode-bridge.md).
///
/// Что трейт **не** фиксирует: протокол общения с внешним процессом, формат
/// сообщений, стратегию перезапуска, разбор конфига, таймауты и
/// версионирование — всё это принадлежит будущей фиче моста.
pub trait EditorModeBridge {
    fn current_mode(&self) -> EditorMode;

    /// Доступен ли полноценный режим и почему нет (FR-034).
    fn availability(&self) -> BridgeAvailability;

    /// Запрашивает переход к выбранному пользователем режиму.
    ///
    /// Обязательство: если полноценный режим недоступен, реализация возвращает
    /// отказ, а не молча оставляет упрощённую эмуляцию, выдавая её за
    /// полноценный режим (FR-034, FR-004).
    fn request_mode(&mut self, preference: super::ModePreference) -> Result<(), BridgeError>;

    /// Применяет конфиг по указанному пути (FR-026, FR-028). Вызывается
    /// **только** по явному действию пользователя: мост не следит за файлом.
    fn apply_config(&mut self, path: &Path, stamp: FileStamp) -> Result<(), BridgeError>;

    /// Сообщает мосту, что пользователь явно решил выйти из `PlainInput`
    /// (FR-035). Самопроизвольного восстановления не бывает.
    fn leave_plain_input(&mut self) -> Result<(), BridgeError>;

    fn poll_events(&mut self) -> Vec<BridgeEvent>;
}

/// Реализация «полноценный режим всегда недоступен с явной причиной».
///
/// На веб-таргете — постоянная (внешние процессы породить нельзя), на native —
/// до появления настоящего моста. Это честная недоступность с причиной, а не
/// заглушка, выдающая себя за работающий режим (FR-034).
#[derive(Debug)]
pub struct NullBridge {
    reason: OutageReason,
    mode: EditorMode,
    events: Vec<BridgeEvent>,
}

impl NullBridge {
    pub fn new(reason: OutageReason) -> Self {
        Self {
            reason,
            mode: EditorMode::SimplifiedEmulation,
            events: Vec::new(),
        }
    }
}

impl Default for NullBridge {
    fn default() -> Self {
        Self::new(OutageReason::BackendNotInstalled)
    }
}

impl EditorModeBridge for NullBridge {
    fn current_mode(&self) -> EditorMode {
        self.mode.clone()
    }

    fn availability(&self) -> BridgeAvailability {
        BridgeAvailability::Unavailable {
            reason: self.reason.clone(),
        }
    }

    fn request_mode(&mut self, preference: super::ModePreference) -> Result<(), BridgeError> {
        match preference {
            super::ModePreference::FullBackend => Err(BridgeError::Unavailable {
                reason: self.reason.clone(),
            }),
            super::ModePreference::SimplifiedEmulation => {
                if self.mode != EditorMode::SimplifiedEmulation {
                    self.mode = EditorMode::SimplifiedEmulation;
                    self.events.push(BridgeEvent::EditorModeChanged {
                        mode: self.mode.clone(),
                    });
                }
                Ok(())
            }
        }
    }

    fn apply_config(&mut self, _path: &Path, _stamp: FileStamp) -> Result<(), BridgeError> {
        Err(BridgeError::Unavailable {
            reason: self.reason.clone(),
        })
    }

    fn leave_plain_input(&mut self) -> Result<(), BridgeError> {
        Err(BridgeError::Unavailable {
            reason: self.reason.clone(),
        })
    }

    fn poll_events(&mut self) -> Vec<BridgeEvent> {
        std::mem::take(&mut self.events)
    }
}

/// Управляемая реализация для тестов: воспроизводит все три состояния и
/// отказные переходы без реального внешнего процесса.
///
/// Объявлена под `#[cfg(any(test, feature = "test-support"))]`, а не под голым
/// `#[cfg(test)]`: тесты фичи живут в `packages/core/tests/` и являются
/// интеграционными — они линкуются с крейтом, собранным без `cfg(test)`, и
/// типа под голым `#[cfg(test)]` попросту не увидели бы (research.md §15).
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug)]
pub struct FakeBridge {
    available: bool,
    reason: OutageReason,
    mode: EditorMode,
    events: Vec<BridgeEvent>,
    config_on_disk: Option<FileStamp>,
    config_readable: bool,
}

#[cfg(any(test, feature = "test-support"))]
impl Default for FakeBridge {
    fn default() -> Self {
        Self::available()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl FakeBridge {
    /// Полноценный режим доступен; текущее состояние — упрощённая эмуляция.
    pub fn available() -> Self {
        Self {
            available: true,
            reason: OutageReason::BackendStopped,
            mode: EditorMode::SimplifiedEmulation,
            events: Vec::new(),
            config_on_disk: None,
            config_readable: true,
        }
    }

    /// Полноценный режим недоступен с указанной причиной.
    pub fn unavailable(reason: OutageReason) -> Self {
        Self {
            available: false,
            reason,
            mode: EditorMode::SimplifiedEmulation,
            events: Vec::new(),
            config_on_disk: None,
            config_readable: true,
        }
    }

    /// Воспроизводит отказ внешнего процесса.
    ///
    /// Единственный переход, происходящий без действия пользователя, и он
    /// ведёт **только** в `PlainInput`: автоматический откат на упрощённую
    /// эмуляцию запрещён (FR-035) — отсутствие vim-раскладки должно быть
    /// однозначно отличимо от штатного режима.
    pub fn fail_backend(&mut self, reason: OutageReason) {
        self.reason = reason.clone();
        self.mode = EditorMode::PlainInput { reason };
        self.events.push(BridgeEvent::EditorModeChanged {
            mode: self.mode.clone(),
        });
    }

    /// Делает файл конфига нечитаемым для следующего `apply_config`.
    pub fn set_config_readable(&mut self, readable: bool) {
        self.config_readable = readable;
    }

    pub fn applied_config_stamp(&self) -> Option<&FileStamp> {
        self.config_on_disk.as_ref()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl EditorModeBridge for FakeBridge {
    fn current_mode(&self) -> EditorMode {
        self.mode.clone()
    }

    fn availability(&self) -> BridgeAvailability {
        if self.available {
            BridgeAvailability::Available
        } else {
            BridgeAvailability::Unavailable {
                reason: self.reason.clone(),
            }
        }
    }

    fn request_mode(&mut self, preference: super::ModePreference) -> Result<(), BridgeError> {
        let next = match preference {
            super::ModePreference::FullBackend => {
                if !self.available {
                    return Err(BridgeError::Unavailable {
                        reason: self.reason.clone(),
                    });
                }
                EditorMode::FullBackend {
                    config: EditorConfig {
                        path: PathBuf::new(),
                        applied_stamp: self.config_on_disk.clone(),
                        disk_state: disk_state(
                            self.config_on_disk.as_ref(),
                            self.config_on_disk.as_ref(),
                        ),
                    },
                }
            }
            super::ModePreference::SimplifiedEmulation => EditorMode::SimplifiedEmulation,
        };
        if self.mode != next {
            self.mode = next;
            self.events.push(BridgeEvent::EditorModeChanged {
                mode: self.mode.clone(),
            });
        }
        Ok(())
    }

    fn apply_config(&mut self, _path: &Path, stamp: FileStamp) -> Result<(), BridgeError> {
        if !self.available {
            return Err(BridgeError::Unavailable {
                reason: self.reason.clone(),
            });
        }
        if !self.config_readable {
            return Err(BridgeError::ConfigUnreadable);
        }
        self.config_on_disk = Some(stamp);
        Ok(())
    }

    fn leave_plain_input(&mut self) -> Result<(), BridgeError> {
        if !self.available {
            return Err(BridgeError::Unavailable {
                reason: self.reason.clone(),
            });
        }
        self.mode = EditorMode::SimplifiedEmulation;
        self.events.push(BridgeEvent::EditorModeChanged {
            mode: self.mode.clone(),
        });
        Ok(())
    }

    fn poll_events(&mut self) -> Vec<BridgeEvent> {
        std::mem::take(&mut self.events)
    }
}

/// Связывает мост, выбранный конфиг и наблюдаемое состояние (T075…T081).
///
/// Существует отдельно от [`EditorModeBridge`] по границе из
/// contracts/editor-mode-bridge.md: «обнаружить, что файл конфига изменился» —
/// забота настроек, «применить конфиг по команде» — моста. Та же семантика
/// «файл действительно изменился» уже реализована в ядре для конфликтов vault,
/// и вторая реализация была бы вторым источником истины (принцип I).
///
/// Контроллер **не читает файл сам**: снимок ему передаёт вызывающая сторона
/// ([`observe_config`](Self::observe_config)). Так он остаётся свободным от
/// платформы — на вебе файла конфига нет вовсе, и подсовывать туда заглушку
/// файловой системы не приходится.
pub struct EditorModeController<B: EditorModeBridge> {
    bridge: B,
    config_path: Option<PathBuf>,
    applied_stamp: Option<FileStamp>,
    disk_state: ConfigDiskState,
    events: Vec<super::events::SettingsEvent>,
}

impl<B: EditorModeBridge> EditorModeController<B> {
    pub fn new(bridge: B) -> Self {
        Self {
            bridge,
            config_path: None,
            applied_stamp: None,
            disk_state: ConfigDiskState::Missing,
            events: Vec::new(),
        }
    }

    pub fn bridge(&self) -> &B {
        &self.bridge
    }

    pub fn bridge_mut(&mut self) -> &mut B {
        &mut self.bridge
    }

    pub fn config_path(&self) -> Option<&Path> {
        self.config_path.as_deref()
    }

    pub fn config_disk_state(&self) -> ConfigDiskState {
        self.disk_state
    }

    pub fn applied_stamp(&self) -> Option<&FileStamp> {
        self.applied_stamp.as_ref()
    }

    /// Текущее наблюдаемое состояние.
    ///
    /// Здесь же держится обязательство FR-034/FR-004: если полноценный режим
    /// недоступен, `FullBackend` не возвращается **ни при каких
    /// обстоятельствах** — даже если мост по ошибке о нём сообщил. Недоступность
    /// не выдаётся за работу.
    pub fn mode(&self) -> EditorMode {
        let mode = self.bridge.current_mode();
        match (&mode, self.bridge.availability()) {
            (EditorMode::FullBackend { .. }, BridgeAvailability::Unavailable { reason }) => {
                EditorMode::PlainInput { reason }
            }
            _ => mode,
        }
    }

    pub fn availability(&self) -> BridgeAvailability {
        self.bridge.availability()
    }

    /// Переключение по выбору пользователя (FR-027).
    pub fn request_mode(&mut self, preference: super::ModePreference) -> Result<(), BridgeError> {
        let before = self.mode();
        let result = self.bridge.request_mode(preference);
        self.emit_mode_if_changed(&before);
        result
    }

    /// Явный выход из `PlainInput` (FR-035). Самопроизвольного восстановления
    /// не бывает: это единственный путь наружу.
    pub fn leave_plain_input(&mut self) -> Result<(), BridgeError> {
        let before = self.mode();
        let result = self.bridge.leave_plain_input();
        self.emit_mode_if_changed(&before);
        result
    }

    /// Указывает путь к конфигу (FR-028). Сам по себе конфиг **не применяется**:
    /// применение — только по явному действию (FR-026).
    pub fn set_config_path(&mut self, path: PathBuf) {
        self.config_path = Some(path);
        self.applied_stamp = None;
        self.set_disk_state(ConfigDiskState::Missing);
    }

    /// Применяет конфиг по явному действию пользователя (FR-026, FR-033).
    ///
    /// `current` — снимок файла, полученный вызывающей стороной. `None`
    /// означает «файла нет или он не читается»: тогда конфиг **не** помечается
    /// применённым, и сообщается причина.
    pub fn apply_config(&mut self, current: Option<FileStamp>) -> Result<(), BridgeError> {
        let Some(path) = self.config_path.clone() else {
            return Err(BridgeError::ConfigMissing);
        };
        let Some(stamp) = current else {
            // FR-033: несуществующий или нечитаемый путь — причина отказа, а не
            // молчаливое «применено».
            self.set_disk_state(ConfigDiskState::Missing);
            return Err(BridgeError::ConfigMissing);
        };

        let before = self.mode();
        self.bridge.apply_config(&path, stamp.clone())?;
        self.applied_stamp = Some(stamp);
        self.set_disk_state(ConfigDiskState::InSync);
        self.emit_mode_if_changed(&before);
        Ok(())
    }

    /// Сообщает контроллеру текущий снимок файла конфига (FR-026).
    ///
    /// Автоматического применения не происходит никогда — только пересчёт
    /// признака «изменён, не применён». Удалённый или нечитаемый конфиг **не
    /// прерывает работу**: полноценный режим продолжает работать на ранее
    /// применённом.
    pub fn observe_config(&mut self, current: Option<FileStamp>) {
        if self.config_path.is_none() {
            return;
        }
        let next = disk_state(self.applied_stamp.as_ref(), current.as_ref());
        self.set_disk_state(next);
    }

    /// Помечает конфиг нечитаемым — отличается от «отсутствует» тем, что файл
    /// на месте, но прочитать его не удалось.
    pub fn observe_config_unreadable(&mut self) {
        if self.config_path.is_none() {
            return;
        }
        self.set_disk_state(ConfigDiskState::Unreadable);
    }

    pub fn poll_events(&mut self) -> Vec<super::events::SettingsEvent> {
        for event in self.bridge.poll_events() {
            match event {
                BridgeEvent::EditorModeChanged { .. } => {
                    // Состояние берётся через `mode()`, а не из события: только
                    // так соблюдается запрет выдавать недоступный режим за
                    // рабочий (FR-034).
                    let mode = self.mode();
                    self.events
                        .push(super::events::SettingsEvent::EditorModeChanged { mode });
                }
            }
        }
        std::mem::take(&mut self.events)
    }

    fn set_disk_state(&mut self, next: ConfigDiskState) {
        if self.disk_state != next {
            self.disk_state = next;
            self.events
                .push(super::events::SettingsEvent::EditorConfigDiskStateChanged {
                    disk_state: next,
                });
        }
    }

    fn emit_mode_if_changed(&mut self, before: &EditorMode) {
        let after = self.mode();
        if &after != before {
            self.events
                .push(super::events::SettingsEvent::EditorModeChanged { mode: after });
        }
    }
}

/// Снимок файла конфига на диске — для вызывающей стороны на native.
/// `None`, если файла нет или он не читается (FR-026, FR-033).
#[cfg(not(target_arch = "wasm32"))]
pub fn stamp_of_config(path: &Path) -> Option<FileStamp> {
    let content = std::fs::read(path).ok()?;
    let mtime_millis = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Some(FileStamp::from_content(mtime_millis, &content))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ModePreference;

    #[test]
    fn null_bridge_never_reports_full_backend() {
        // FR-034, FR-004: недоступность не выдаётся за работу.
        let mut bridge = NullBridge::new(OutageReason::ExternalProcessesUnavailable);
        assert!(!bridge.current_mode().is_full_backend());
        assert!(bridge.request_mode(ModePreference::FullBackend).is_err());
        assert!(!bridge.current_mode().is_full_backend());
    }

    #[test]
    fn backend_failure_lands_in_plain_input_never_in_emulation() {
        // FR-035, contracts/editor-mode-bridge.md §1.
        let mut bridge = FakeBridge::available();
        bridge.request_mode(ModePreference::FullBackend).unwrap();
        assert!(bridge.current_mode().is_full_backend());

        bridge.fail_backend(OutageReason::BackendStopped);

        assert_eq!(
            bridge.current_mode(),
            EditorMode::PlainInput {
                reason: OutageReason::BackendStopped
            }
        );
        assert_ne!(bridge.current_mode(), EditorMode::SimplifiedEmulation);
    }

    #[test]
    fn plain_input_is_left_only_by_explicit_action() {
        // FR-035: самопроизвольного восстановления нет.
        let mut bridge = FakeBridge::available();
        bridge.fail_backend(OutageReason::BackendStopped);
        let _ = bridge.poll_events();

        // Время идёт, событий нет, состояние прежнее.
        assert!(bridge.poll_events().is_empty());
        assert!(matches!(
            bridge.current_mode(),
            EditorMode::PlainInput { .. }
        ));

        bridge.leave_plain_input().unwrap();
        assert_eq!(bridge.current_mode(), EditorMode::SimplifiedEmulation);
    }

    #[test]
    fn config_returned_to_previous_content_is_in_sync_again() {
        // research.md §8: сравнение по хэшу, не по времени.
        let applied = FileStamp::from_content(100, b"set number");
        let edited = FileStamp::from_content(200, b"set nonumber");
        let reverted = FileStamp::from_content(300, b"set number");

        assert_eq!(
            disk_state(Some(&applied), Some(&edited)),
            ConfigDiskState::Changed
        );
        assert_eq!(
            disk_state(Some(&applied), Some(&reverted)),
            ConfigDiskState::InSync
        );
        assert_eq!(disk_state(Some(&applied), None), ConfigDiskState::Missing);
    }

    #[test]
    fn unreadable_config_is_not_marked_applied() {
        // FR-033.
        let mut bridge = FakeBridge::available();
        bridge.set_config_readable(false);
        let result = bridge.apply_config(
            Path::new("/nowhere/init.lua"),
            FileStamp::from_content(1, b"x"),
        );
        assert_eq!(result, Err(BridgeError::ConfigUnreadable));
        assert!(bridge.applied_config_stamp().is_none());
    }
}
