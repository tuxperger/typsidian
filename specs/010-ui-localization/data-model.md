# Data Model: Мультиязычный интерфейс (фича 010)

## LanguageChoice — выбор пользователя (ядро, настройки устройства)

| Значение | В `settings.json` | Смысл |
|---|---|---|
| `System` | ключ `language` отсутствует | язык определяется по системе (FR-008) |
| `Tag(String)` | `"language": "en"` | явный выбор (FR-010) |

- Хранится в `Settings.language: Option<String>`; `None` ⇔ `System`.
- **Валидация (ядро)**: только синтаксис тега — `^[A-Za-z]{2,8}(-[A-Za-z0-9]{1,8})*$`,
  ≤35 знаков. Неверный синтаксис → `InvalidSettings::LanguageTag`, набор отклоняется целиком
  (как для размера шрифта).
- **Поддерживаемость** ядро не проверяет: нераспознанный, но синтаксически верный тег
  хранится и не перезаписывается, пока пользователь не выберет язык явно (FR-018).
- **Маршрут (решает ядро)**: язык всегда уходит в хранилище устройства, даже при открытом
  vault (FR-016). Новая функция ядра `route(change, vault_is_open) -> Vec<RoutedChange>`
  раскладывает набор на части по хранилищам: набор «язык + тема» при открытом vault даёт две
  записи — `{Device, language}` и `{OpenVault, theme}`. UI выполняет записи по порядку и сам
  ничего не решает (принцип I). `target_for` остаётся для наборов без языка; одна **запись**
  по-прежнему не пишет в оба хранилища (006 FR-029).
- **Чтение файла vs. изменение**: при разборе `settings.json` синтаксически неверный тег не
  является ошибкой — он хранится как нераспознанный (FR-018), остальные значения читаются как
  обычно. `InvalidSettings::LanguageTag` возникает только у `SettingsChange` с неверным тегом.
- **Сброс** (`reset_to_defaults`): `System` (FR-017).
- `device_defaults_change` язык не трогает: он и так живёт только на устройстве.

## Language — поддерживаемый язык (UI)

| Поле | Тип | Пример |
|---|---|---|
| `code` | основной подтег BCP 47 | `"ru"` |
| `nativeName` | название на самом языке | `"Русский"` |
| `catalog` | `Catalog` или `Partial<Catalog>` | — |
| `engineLang` | код языка для редактора досок, если поддерживается | `"ru-RU"` |

- Поставляемые языки (`en`, `ru`) объявлены полным `Catalog` — пропуск ключа ломает `tsc`.
- Новый язык может быть объявлен `Partial<Catalog>`: недостающие ключи берутся из `en`
  (FR-027). Выпуск с неполным каталогом отмечается в реестре языков явно (`complete: false`)
  и попадает в вывод `check-i18n.sh` предупреждением.

## Catalog — набор переводов (UI)

- Тип выводится из `en`: `type Catalog = { [K in keyof typeof en]: Widen<typeof en[K]> }`.
- Значение ключа: `string` или `(params: P) => string`. Параметры типизированы; имена
  пользователя подставляются как есть (FR-005).
- Пространства ключей повторяют раскладку `ui/src`: `tree.*`, `editor.*`, `preview.*`,
  `boards.*`, `settings.*`, `templates.*`, `export.*`, `search.*`, `palette.*`, `menu.*`,
  `shell.*`, `errors.*`, `app.*` (коды `AppMessage`).

## AppMessage — сообщение, сформированное приложением (ядро → UI)

`#[serde(tag = "code")]`, типы порождаются tsify. Варианты (инвентаризация на 2026-09-29):

| code | Параметры | Откуда |
|---|---|---|
| `DependencyUnreadable` | `path`, `reason: ResolveReason` | `with_resolve_failures` |
| `TemplateMissing` | `spec` | `ResolveError::PackageMissing` |
| `TemplateDownloaded` | `spec`, `url`, `fileCount`, `fingerprint` | `build_installing_templates` |
| `TemplateInstallFailed` | `spec`, `reason: InstallFailure` | то же |
| `TemplateFingerprintDiverged` | `spec`, `recorded`, `actual` | `fingerprint_notes` |
| `VaultFontUnusable` | `path`, `reason: FontProblem` | `with_font_problems` |
| `ExportFailed` | `reason: ExportFailure` | `export/mod.rs` |

- `ResolveReason`: `NotFound | Unreadable | AccessDenied | OutsideVault | TooLarge`.
- `InstallFailure`: `Offline{url} | SourceUnavailable{url} | VersionNotFound{spec} | Timeout{url} | ArchiveRejected{reason} | VaultNotWritable | AutoDownloadDisabled{spec} | …` — один к одному с `InstallError`.
- `FontProblem`: `Unreadable{reason: ResolveReason} | Invalid`.
- `ExportFailure` — один к одному с отказами экспорта и `ExportFolderError`.

## Diagnostic (изменение)

| Поле | Было | Стало |
|---|---|---|
| `message` | текст для показа | языконезависимый текст для журналов; для диагностик Typst — сообщение компилятора, показывается как есть |
| `app` | — | `Option<AppMessage>`; `Some` ⇔ сообщение сформировано приложением |

Инвариант: `app.is_some()` ⇒ UI не показывает `message`. `app.is_none()` ⇒ это сообщение
Typst (FR-004).

## Уже структурные отказы (без изменений формы)

`OpenError`, `ReadError`, `WriteError`, `CreateError`, `RenameError`, `DeleteError`,
`ResolveError` (vault), `FolderAccess`/`FolderProblem`, `RejectReason`, отказы настроек vault —
UI сопоставляет их `kind` с ключами `errors.*`. Показ `error.message` в UI (сегодня:
`OpenVault.tsx`, `useVaultList.ts`) заменяется сопоставлением по `kind`.

## Состояния

```text
старт ─► чтение настроек устройства ─┬─ ok: choice ─► resolveLanguage ─► язык
                                     └─ отказ ──────► resolveLanguage(System) ─► язык (FR-029)
смена в панели ─► SettingsChange{language} ─► Device store ─► контекст ─► перерисовка (≤1 с)
```
