//! Планирование применения изменений (research.md §9, FR-029…FR-032, FR-037).
//!
//! Здесь живёт решение «можно ли применить набор», куда его писать и как он
//! меняет значения. Это бизнес-логика, и ей место в ядре (принцип I).

use super::{InvalidSettings, Settings, Theme, VaultId};
use serde::{Deserialize, Serialize};

/// Набор изменений, применяемых как одно целое (research.md §9). Поле со
/// значением `None` означает «не трогать».
///
/// Сброс к умолчаниям (FR-037) строит полный набор и проходит ровно тот же
/// путь — отдельной операции сброса, обходящей предусловия, не существует.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SettingsChange {
    pub theme: Option<Theme>,
    pub editor_font_size: Option<u8>,
    pub current_vault: Option<VaultId>,
    /// Язык интерфейса (фича 010). Принадлежит устройству при любом раскладе
    /// (FR-016): см. [`route`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub language: Option<LanguageChoice>,
    /// Тождество устройства (фича 011). Как и язык — только устройству.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub device: Option<crate::sync::DeviceIdentity>,
    /// Значения синхронизации устройства для одного vault (фича 011):
    /// заменяют прежнюю запись этого vault целиком.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub vault_sync: Option<crate::sync::DeviceVaultSyncEntry>,
    /// Состояние колонки заметки (фича 013, FR-021). Всегда в устройство: см.
    /// [`route`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub note_panel: Option<super::NotePanelState>,
    /// Набор вкладок одного vault (фича 014, FR-022): заменяет прежнюю запись
    /// этого vault целиком. Всегда в устройство: см. [`route`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub workspace: Option<super::VaultWorkspaceEntry>,
    /// Предлагать ли метки и заметки при наборе (фича 018). Всегда в
    /// устройство: см. [`route`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub editor_completion_auto: Option<bool>,
}

/// Выбор языка интерфейса (фича 010, data-model.md «LanguageChoice»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum LanguageChoice {
    /// Как в системе: язык определяется интерфейсом по языкам системы.
    System,
    /// Явный выбор — тег языка (`"en"`, `"ru"`).
    Tag(String),
}

impl SettingsChange {
    pub fn language(choice: LanguageChoice) -> Self {
        Self {
            language: Some(choice),
            ..Self::default()
        }
    }

    pub fn note_panel(state: super::NotePanelState) -> Self {
        Self {
            note_panel: Some(state),
            ..Self::default()
        }
    }

    pub fn theme(theme: Theme) -> Self {
        Self {
            theme: Some(theme),
            ..Self::default()
        }
    }

    pub fn font_size(value: u8) -> Self {
        Self {
            editor_font_size: Some(value),
            ..Self::default()
        }
    }

    pub fn current_vault(id: VaultId) -> Self {
        Self {
            current_vault: Some(id),
            ..Self::default()
        }
    }

    /// Меняет ли набор что-то, кроме выбора текущего vault.
    ///
    /// Нужно маршрутизации: набор, состоящий **только** из `current_vault`,
    /// целиком принадлежит устройству и в vault не пишется никогда (FR-028).
    pub fn touches_vault_scoped_values(&self) -> bool {
        self.theme.is_some() || self.editor_font_size.is_some()
    }

    /// Применяет набор к копии значений. Список известных vault не трогается
    /// никогда: он не входит в набор изменений (FR-038).
    pub fn apply_to(&self, base: &Settings) -> Settings {
        let mut next = base.clone();
        if let Some(theme) = self.theme {
            next.theme = theme;
        }
        if let Some(size) = self.editor_font_size {
            next.editor_font_size = size;
        }
        if let Some(vault) = &self.current_vault {
            next.current_vault = Some(vault.clone());
        }
        match &self.language {
            Some(LanguageChoice::System) => next.language = None,
            Some(LanguageChoice::Tag(tag)) => next.language = Some(tag.clone()),
            None => {}
        }
        if let Some(device) = &self.device {
            next.device = Some(device.clone());
        }
        if let Some(state) = self.note_panel {
            next.note_panel = state;
        }
        if let Some(auto) = self.editor_completion_auto {
            next.editor_completion_auto = auto;
        }
        if let Some(entry) = &self.workspace {
            next.workspaces
                .retain(|existing| existing.vault != entry.vault);
            // Набор без вкладок хранить незачем: запись исчезает.
            if !entry.value.is_empty() {
                next.workspaces.push(entry.clone());
            }
        }
        if let Some(entry) = &self.vault_sync {
            next.vault_sync
                .retain(|existing| existing.vault != entry.vault);
            let value = entry.value.clone().normalized();
            if value != crate::sync::DeviceVaultSync::default() {
                next.vault_sync.push(crate::sync::DeviceVaultSyncEntry {
                    vault: entry.vault.clone(),
                    value,
                });
            }
        }
        next
    }
}

/// Полный набор изменений, возвращающий значения к умолчаниям (FR-037).
///
/// `current_vault` в него не входит намеренно: сброс не должен закрывать
/// открытый vault и не трогает список известных (FR-038, SC-011).
pub fn reset_to_defaults() -> SettingsChange {
    let defaults = Settings::default();
    SettingsChange {
        theme: Some(defaults.theme),
        editor_font_size: Some(defaults.editor_font_size),
        current_vault: None,
        // FR-017: сброс возвращает «как в системе».
        language: Some(LanguageChoice::System),
        // Тождество устройства и значения синхронизации — не настройки
        // внешнего вида: сброс их не трогает (фича 011).
        ..SettingsChange::default()
    }
}

/// Набор изменений, делающий **действующие сейчас** значения значениями
/// устройства по умолчанию (фича 006, FR-031).
///
/// Применяется к хранилищу устройства и только к нему: это единственный способ
/// изменить умолчания, потому что обычная правка при открытом vault уходит в
/// сам vault и устройства не касается (FR-029). Отсюда предсказуемость: папка,
/// у которой своих значений нет, открывается ровно с тем, что пользователь
/// объявил умолчанием, а не с тем, что он последний раз где-то настроил.
///
/// `effective` — уже разрешённые слои ([`super::resolve`]), а не перекрытия
/// vault: «сделать по умолчанию» означает «пусть устройство считает так же,
/// как здесь», включая значения, которые сюда попали с самого устройства.
///
/// `current_vault` в набор не входит: выбор открытой папки умолчанием не
/// является, и переносить его в этой операции незачем (FR-028).
pub fn device_defaults_change(effective: &Settings) -> SettingsChange {
    SettingsChange {
        theme: Some(effective.theme),
        editor_font_size: Some(effective.editor_font_size),
        current_vault: None,
        // Язык и так живёт только на устройстве — переносить нечего.
        language: None,
        ..SettingsChange::default()
    }
}

/// Почему изменение не применено целиком.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum RejectReason {
    #[error("значение вне допустимого диапазона")]
    ValueOutOfRange,
    #[error("хранилище настроек недоступно на запись")]
    StorageNotWritable,
}

impl From<InvalidSettings> for RejectReason {
    fn from(error: InvalidSettings) -> Self {
        match error {
            InvalidSettings::FontSizeOutOfRange { .. } => RejectReason::ValueOutOfRange,
            // Ссылка на несуществующую запись и дубль в списке — тоже
            // непригодные значения; отдельного варианта спецификация не
            // вводит.
            InvalidSettings::CurrentVaultNotKnown | InvalidSettings::DuplicateVaultId => {
                RejectReason::ValueOutOfRange
            }
            InvalidSettings::LanguageTag { .. } => RejectReason::ValueOutOfRange,
        }
    }
}

/// Результат планирования в ядре (data-model.md «ApplyPlan»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "outcome")]
pub enum ApplyPlan {
    /// Предусловий нет — изменения можно записывать атомарно.
    Applied,
    /// Изменение не применено целиком — **ни одна** настройка из набора не
    /// меняется.
    Rejected { reason: RejectReason },
}

/// В какое из двух хранилищ уходит набор изменений (фича 006, FR-029, FR-031).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "target")]
pub enum ChangeTarget {
    /// `.typsidian/vault.json` открытого vault.
    OpenVault,
    /// `settings.json` устройства.
    Device,
}

/// Куда записать набор изменений (фича 006, FR-029).
///
/// **В оба хранилища одно действие не пишет никогда.** Именно это делает
/// значения устройства предсказуемыми: пользователь, покрасивший один vault в
/// тёмное, не меняет умолчание для всех остальных. Действие «сделать
/// значениями по умолчанию» — отдельная операция, а не вариант этой, и потому
/// пишет в устройство по построению, а не по ветке здесь.
///
/// Выбор текущего vault и список известных остаются на устройстве при любом
/// раскладе (FR-028): набор, который их трогает, в vault не уйдёт.
pub fn target_for(change: &SettingsChange, vault_is_open: bool) -> ChangeTarget {
    if !vault_is_open {
        // Перекрывать нечего: vault не открыт.
        return ChangeTarget::Device;
    }
    if change.touches_vault_scoped_values() {
        ChangeTarget::OpenVault
    } else {
        // Остался только `current_vault` — свойство устройства.
        ChangeTarget::Device
    }
}

/// Часть набора изменений вместе с хранилищем, куда её писать (фича 010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RoutedChange {
    pub target: ChangeTarget,
    pub change: SettingsChange,
}

/// Раскладывает набор изменений по хранилищам (фича 010, FR-016).
///
/// Язык интерфейса — свойство устройства при любом раскладе: он не переезжает
/// вместе с папкой заметок. Набор «язык + тема» при открытом vault поэтому
/// даёт **две** записи — язык в устройство, тему в vault, — и каждая по-прежнему
/// пишет ровно в одно хранилище (006 FR-029).
///
/// Решение здесь, а не в интерфейсе (принцип I): панель исполняет записи по
/// порядку и сама не знает, какое значение где живёт. Пустой набор даёт пустой
/// список.
pub fn route(change: &SettingsChange, vault_is_open: bool) -> Vec<RoutedChange> {
    let mut rest = change.clone();
    let language = rest.language.take();
    // Тождество и значения синхронизации устройства (фича 011) — тоже только
    // устройству, по той же причине, что язык.
    let device = rest.device.take();
    let vault_sync = rest.vault_sync.take();
    // Состояние колонки заметки (фича 013) — тоже только устройству.
    let note_panel = rest.note_panel.take();
    // Набор вкладок (фича 014, FR-022) — тоже только устройству: вне vault, вне
    // его истории и доставки.
    let workspace = rest.workspace.take();
    // Дополнение при наборе (фича 018) — привычка устройства.
    let editor_completion_auto = rest.editor_completion_auto.take();
    let mut routed = Vec::with_capacity(2);
    if language.is_some()
        || device.is_some()
        || vault_sync.is_some()
        || note_panel.is_some()
        || workspace.is_some()
        || editor_completion_auto.is_some()
    {
        routed.push(RoutedChange {
            target: ChangeTarget::Device,
            change: SettingsChange {
                language,
                device,
                vault_sync,
                note_panel,
                workspace,
                editor_completion_auto,
                ..SettingsChange::default()
            },
        });
    }
    if rest != SettingsChange::default() {
        routed.push(RoutedChange {
            target: target_for(&rest, vault_is_open),
            change: rest,
        });
    }
    routed
}

/// Планирует применение набора (research.md §9): непригодное значение
/// отклоняет набор целиком, и ни одна настройка из него не меняется.
pub fn plan(base: &Settings, change: &SettingsChange) -> ApplyPlan {
    // Тег проверяется у **нового** значения, а не у кандидата целиком: язык,
    // вписанный в файл руками или пришедший из более новой версии, не должен
    // блокировать смену темы (FR-018).
    if let Some(LanguageChoice::Tag(tag)) = &change.language {
        if !super::is_valid_language_tag(tag) {
            return ApplyPlan::Rejected {
                reason: super::InvalidSettings::LanguageTag { value: tag.clone() }.into(),
            };
        }
    }
    let candidate = change.apply_to(base);
    if let Err(invalid) = super::validate(&candidate) {
        return ApplyPlan::Rejected {
            reason: invalid.into(),
        };
    }
    ApplyPlan::Applied
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Settings {
        Settings::default()
    }

    #[test]
    fn a_change_at_an_open_vault_goes_to_the_vault() {
        // FR-029: и **не** в значения устройства. Именно это делает их
        // предсказуемыми при нескольких открытых по очереди папках.
        assert_eq!(
            target_for(&SettingsChange::theme(Theme::Dark), true),
            ChangeTarget::OpenVault
        );
        assert_eq!(
            target_for(&SettingsChange::font_size(18), true),
            ChangeTarget::OpenVault
        );
    }

    #[test]
    fn without_an_open_vault_there_is_nothing_to_override() {
        assert_eq!(
            target_for(&SettingsChange::theme(Theme::Dark), false),
            ChangeTarget::Device
        );
    }

    #[test]
    fn making_current_values_the_device_defaults_carries_them_all() {
        // FR-031: устройство начинает считать так же, как открытый vault.
        let effective = Settings {
            theme: Theme::Dark,
            editor_font_size: 18,
            ..Settings::default()
        };
        let change = device_defaults_change(&effective);

        assert_eq!(change.theme, Some(Theme::Dark));
        assert_eq!(change.editor_font_size, Some(18));
        // Выбор открытой папки умолчанием не является (FR-028).
        assert_eq!(change.current_vault, None);
    }

    #[test]
    fn choosing_the_current_vault_never_goes_into_a_vault() {
        // FR-028: список известных vault и текущий — свойство устройства, и
        // класть их внутрь одной из папок нельзя даже при открытом vault.
        assert_eq!(
            target_for(&SettingsChange::current_vault(VaultId::from_raw("a")), true),
            ChangeTarget::Device
        );
    }

    #[test]
    fn plain_value_change_is_applied_without_preconditions() {
        let plan = plan(&base(), &SettingsChange::theme(Theme::Dark));
        assert_eq!(plan, ApplyPlan::Applied);
    }

    #[test]
    fn font_size_out_of_range_is_rejected() {
        // FR-016, research.md §11.
        for value in [7u8, 33u8] {
            assert_eq!(
                plan(&base(), &SettingsChange::font_size(value)),
                ApplyPlan::Rejected {
                    reason: RejectReason::ValueOutOfRange
                }
            );
        }
        for value in [8u8, 32u8] {
            assert_eq!(
                plan(&base(), &SettingsChange::font_size(value)),
                ApplyPlan::Applied
            );
        }
    }

    #[test]
    fn reset_returns_values_to_defaults_but_keeps_known_vaults() {
        // FR-037, FR-038, SC-011.
        let record =
            super::super::VaultRecord::new(VaultId::from_raw("a"), "/work/notes".to_string());
        let current = Settings {
            theme: Theme::Dark,
            editor_font_size: 22,
            known_vaults: vec![record.clone()],
            current_vault: Some(VaultId::from_raw("a")),
            language: Some("en".to_string()),
            ..Settings::default()
        };

        let change = reset_to_defaults();
        assert_eq!(plan(&current, &change), ApplyPlan::Applied);

        let after = change.apply_to(&current);
        assert_eq!(after.theme, Theme::Light);
        assert_eq!(after.editor_font_size, 14);
        assert_eq!(
            after.known_vaults,
            vec![record],
            "сброс не трогает список известных vault"
        );
        assert_eq!(
            after.current_vault,
            Some(VaultId::from_raw("a")),
            "сброс не закрывает открытый vault"
        );
    }

    #[test]
    fn change_applies_only_named_fields() {
        let base = Settings {
            theme: Theme::Dark,
            editor_font_size: 20,
            ..Settings::default()
        };
        let next = SettingsChange::theme(Theme::Light).apply_to(&base);
        assert_eq!(next.theme, Theme::Light);
        assert_eq!(next.editor_font_size, 20, "нетронутое поле не меняется");
    }
}
