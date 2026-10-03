# Contract: сообщения и язык в ядре

## `typsidian_core::messages::AppMessage`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[serde(tag = "code", rename_all_fields = "camelCase")]
pub enum AppMessage { /* варианты — data-model.md § AppMessage */ }

impl AppMessage {
    /// Языконезависимый текст для журналов и тестов (FR-031). В интерфейс не идёт.
    pub fn log_text(&self) -> String;
}
```

- Параметры несут значения пользователя без изменений (FR-005).
- Сериализация одинакова на native и wasm32 (принцип II): тест сверяет JSON каждого
  варианта на обоих таргетах.

## `Diagnostic`

```rust
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,                 // Typst: текст компилятора; приложение: log_text()
    pub location: Option<DiagnosticLocation>,
    pub template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<AppMessage>,         // новое
}
```

## Настройки

```rust
pub struct Settings { /* … */ pub language: Option<String> }

pub enum LanguageChoice { System, Tag(String) }   // serde: "system" | { "tag": "en" }

pub struct SettingsChange { /* … */ pub language: Option<LanguageChoice> }

pub enum InvalidSettings { /* … */ LanguageTag { value: String } }
```

- `route(change, vault_is_open) -> Vec<RoutedChange { target: ChangeTarget, change: SettingsChange }>` —
  раскладка набора по хранилищам в ядре: язык всегда `Device`; «язык + тема» при открытом
  vault → `[Device{language}, OpenVault{theme}]`. UI выполняет записи и ничего не решает.
- `target_for(change с одним language, vault_is_open: true)` → `ChangeTarget::Device`.
- Кривой тег при разборе `settings.json` — не ошибка файла: значение хранится, остальные
  ключи читаются; `InvalidSettings::LanguageTag` — только у `SettingsChange`.
- `reset_to_defaults().language == Some(LanguageChoice::System)`.
- `format::render` пишет/удаляет ключ `"language"`; нераспознанный тег сохраняется.

## Оболочка (`packages/shell`)

Команды настроек устройства уже передают `Settings`/`SettingsChange` целиком; новое поле
проходит без правок. Новая команда — одна: `route_settings_change` рядом с существующей
командой `target_for` (`packages/shell/src/lib.rs`); web получает тот же `route` через
биндинг в `packages/core/src/settings/web.rs`.
