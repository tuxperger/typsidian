//! Планирование применения изменений (research.md §9, FR-029…FR-032, FR-037).
//!
//! Здесь живёт решение «можно ли сейчас применить набор и что для этого должна
//! сделать вызывающая сторона». Это бизнес-логика, и ей место в ядре
//! (принцип I): UI исполняет возвращённую инструкцию, а не проверяет сам, есть
//! ли конфликт и нужно ли сохранять.
//!
//! Модуль намеренно **не вызывает** `VaultBackend`: сохранение заметки требует
//! vault-бэкенда, и связывать с ним хранилище настроек означало бы сцепить две
//! независимые подсистемы. Вместо этого ядро возвращает
//! [`ApplyPlan::RequiresNoteSave`], а исполняет её вызывающая сторона
//! существующим примитивом `VaultBackend::write`.

use super::{InvalidSettings, ModePreference, Settings, Theme, VaultId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub editor_mode_preference: Option<ModePreference>,
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string | undefined"))]
    pub editor_config_path: Option<PathBuf>,
}

impl SettingsChange {
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

    pub fn mode_preference(preference: ModePreference) -> Self {
        Self {
            editor_mode_preference: Some(preference),
            ..Self::default()
        }
    }

    pub fn current_vault(id: VaultId) -> Self {
        Self {
            current_vault: Some(id),
            ..Self::default()
        }
    }

    /// Меняет ли набор режим редактирования — именно это включает предусловия
    /// FR-029…FR-031.
    pub fn touches_editor_mode(&self) -> bool {
        self.editor_mode_preference.is_some()
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
        if let Some(preference) = self.editor_mode_preference {
            next.editor_mode_preference = preference;
        }
        if let Some(path) = &self.editor_config_path {
            next.editor_config_path = Some(path.clone());
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
        editor_mode_preference: Some(defaults.editor_mode_preference),
        editor_config_path: None,
    }
}

/// Почему изменение не применено целиком.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum RejectReason {
    #[error("не удалось сохранить открытую заметку")]
    SaveFailed,
    #[error("по открытой заметке есть неразрешённый конфликт")]
    UnresolvedConflict,
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
    /// Меняется режим редактирования при открытой заметке с правками:
    /// вызывающая сторона обязана сохранить её и повторить вызов, сопроводив
    /// это сообщением пользователю (FR-029).
    RequiresNoteSave {
        #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
        path: PathBuf,
    },
    /// Изменение не применено целиком — **ни одна** настройка из набора не
    /// меняется, включая не связанные с режимом (FR-030, FR-031).
    Rejected { reason: RejectReason },
}

/// Контекст, в котором принимается решение. Собирается вызывающей стороной:
/// ядро настроек не имеет доступа ни к редактору, ни к vault-бэкенду.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyContext {
    /// Заметка с несохранёнными правками, открытая прямо сейчас.
    pub open_note_with_unsaved_edits: Option<PathBuf>,
    /// По открытой заметке есть неразрешённый конфликт (FR-031).
    pub has_unresolved_conflict: bool,
    /// Предыдущая попытка сохранить заметку провалилась (FR-030).
    pub last_save_failed: bool,
    /// Хранилище доступно на запись. `false` не блокирует изменение: оно
    /// действует до конца сессии (FR-014) — отказ здесь означал бы, что
    /// приложение теряет настраиваемость из-за прав на каталог.
    pub storage_writable: bool,
}

impl ApplyContext {
    /// Контекст «ничего не мешает»: заметок с правками нет, хранилище пишется.
    pub fn unobstructed() -> Self {
        Self {
            storage_writable: true,
            ..Self::default()
        }
    }
}

/// Планирует применение набора (research.md §9).
///
/// Порядок проверок значим. Сначала значения: непригодное значение отклоняется
/// независимо от состояния редактора, иначе пользователь получил бы требование
/// сохранить заметку ради изменения, которое всё равно будет отвергнуто.
/// Затем — предусловия смены режима, и только те, что относятся к режиму:
/// смена темы при неразрешённом конфликте ничем не мешает и не блокируется.
pub fn plan(base: &Settings, change: &SettingsChange, context: &ApplyContext) -> ApplyPlan {
    let candidate = change.apply_to(base);
    if let Err(invalid) = super::validate(&candidate) {
        return ApplyPlan::Rejected {
            reason: invalid.into(),
        };
    }

    if change.touches_editor_mode() {
        // FR-031: неразрешённый конфликт блокирует переключение целиком, и ни
        // одной записи в файл не происходит.
        if context.has_unresolved_conflict {
            return ApplyPlan::Rejected {
                reason: RejectReason::UnresolvedConflict,
            };
        }
        // FR-030: провалившееся сохранение блокирует переключение — иначе
        // выбор «сохранить и переключить» терял бы данные ровно в том случае,
        // ради которого он и сделан.
        if context.last_save_failed {
            return ApplyPlan::Rejected {
                reason: RejectReason::SaveFailed,
            };
        }
        // FR-029: сохранение не молчаливое — ядро возвращает инструкцию, а
        // вызывающая сторона исполняет её и сообщает пользователю.
        if let Some(path) = &context.open_note_with_unsaved_edits {
            return ApplyPlan::RequiresNoteSave { path: path.clone() };
        }
        // FR-032: без открытой заметки смена режима происходит немедленно, без
        // шага сохранения.
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
    fn plain_value_change_is_applied_without_preconditions() {
        let plan = plan(
            &base(),
            &SettingsChange::theme(Theme::Dark),
            &ApplyContext::unobstructed(),
        );
        assert_eq!(plan, ApplyPlan::Applied);
    }

    #[test]
    fn font_size_out_of_range_is_rejected() {
        // FR-016, research.md §11.
        for value in [7u8, 33u8] {
            assert_eq!(
                plan(
                    &base(),
                    &SettingsChange::font_size(value),
                    &ApplyContext::unobstructed()
                ),
                ApplyPlan::Rejected {
                    reason: RejectReason::ValueOutOfRange
                }
            );
        }
        for value in [8u8, 32u8] {
            assert_eq!(
                plan(
                    &base(),
                    &SettingsChange::font_size(value),
                    &ApplyContext::unobstructed()
                ),
                ApplyPlan::Applied
            );
        }
    }

    #[test]
    fn mode_change_with_dirty_note_requires_save_first() {
        // FR-029.
        let context = ApplyContext {
            open_note_with_unsaved_edits: Some(PathBuf::from("note.typ")),
            ..ApplyContext::unobstructed()
        };
        assert_eq!(
            plan(
                &base(),
                &SettingsChange::mode_preference(ModePreference::FullBackend),
                &context
            ),
            ApplyPlan::RequiresNoteSave {
                path: PathBuf::from("note.typ")
            }
        );
    }

    #[test]
    fn mode_change_without_open_note_applies_immediately() {
        // FR-032.
        assert_eq!(
            plan(
                &base(),
                &SettingsChange::mode_preference(ModePreference::FullBackend),
                &ApplyContext::unobstructed()
            ),
            ApplyPlan::Applied
        );
    }

    #[test]
    fn unresolved_conflict_blocks_mode_change_entirely() {
        // FR-031.
        let context = ApplyContext {
            has_unresolved_conflict: true,
            open_note_with_unsaved_edits: Some(PathBuf::from("note.typ")),
            ..ApplyContext::unobstructed()
        };
        assert_eq!(
            plan(
                &base(),
                &SettingsChange::mode_preference(ModePreference::FullBackend),
                &context
            ),
            ApplyPlan::Rejected {
                reason: RejectReason::UnresolvedConflict
            }
        );
    }

    #[test]
    fn failed_save_blocks_mode_change() {
        // FR-030.
        let context = ApplyContext {
            last_save_failed: true,
            open_note_with_unsaved_edits: Some(PathBuf::from("note.typ")),
            ..ApplyContext::unobstructed()
        };
        assert_eq!(
            plan(
                &base(),
                &SettingsChange::mode_preference(ModePreference::FullBackend),
                &context
            ),
            ApplyPlan::Rejected {
                reason: RejectReason::SaveFailed
            }
        );
    }

    #[test]
    fn conflict_does_not_block_changes_unrelated_to_mode() {
        // Предусловия FR-029…FR-031 относятся к смене режима, а не ко всему
        // подряд: тема при неразрешённом конфликте меняется свободно.
        let context = ApplyContext {
            has_unresolved_conflict: true,
            open_note_with_unsaved_edits: Some(PathBuf::from("note.typ")),
            ..ApplyContext::unobstructed()
        };
        assert_eq!(
            plan(&base(), &SettingsChange::theme(Theme::Dark), &context),
            ApplyPlan::Applied
        );
    }

    #[test]
    fn reset_returns_values_to_defaults_but_keeps_known_vaults() {
        // FR-037, FR-038, SC-011.
        let record = super::super::VaultRecord::new(
            VaultId::from_raw("a"),
            "/work/notes".to_string(),
        );
        let current = Settings {
            theme: Theme::Dark,
            editor_font_size: 22,
            known_vaults: vec![record.clone()],
            current_vault: Some(VaultId::from_raw("a")),
            editor_mode_preference: ModePreference::FullBackend,
            editor_config_path: Some(PathBuf::from("/home/user/init.lua")),
        };

        let change = reset_to_defaults();
        assert_eq!(
            plan(&current, &change, &ApplyContext::unobstructed()),
            ApplyPlan::Applied
        );

        let after = change.apply_to(&current);
        assert_eq!(after.theme, Theme::Light);
        assert_eq!(after.editor_font_size, 14);
        assert_eq!(
            after.editor_mode_preference,
            ModePreference::SimplifiedEmulation
        );
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
    fn reset_is_all_or_nothing_when_a_precondition_blocks_it() {
        // FR-037, кларификация 2026-08-09: сброс проходит те же предусловия,
        // что и смена режима, и при отказе не меняется ни одна настройка.
        let current = Settings {
            theme: Theme::Dark,
            editor_font_size: 22,
            ..Settings::default()
        };
        let context = ApplyContext {
            has_unresolved_conflict: true,
            open_note_with_unsaved_edits: Some(PathBuf::from("note.typ")),
            ..ApplyContext::unobstructed()
        };
        assert_eq!(
            plan(&current, &reset_to_defaults(), &context),
            ApplyPlan::Rejected {
                reason: RejectReason::UnresolvedConflict
            }
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
        assert!(!SettingsChange::theme(Theme::Light).touches_editor_mode());
    }
}
