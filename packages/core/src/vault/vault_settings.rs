//! Настройки уровня vault — файл `.typsidian/vault.json` (фича 006,
//! contracts/vault-settings.md). Формат — JSON, как и у `settings.json`
//! устройства; почему именно он и чем за это заплачено, изложено в
//! `settings::format`.
//!
//! # Почему здесь нет трейта
//!
//! Функции поверх [`VaultBackend`], а не третья реализация
//! `settings::store::SettingsStore`. Приём тот же, что в `boards::store`:
//! логика платформенно нейтральна, а всё, что различается по платформам, —
//! чтение и запись файлов, — уже скрыто трейтом из фичи 001.
//!
//! `SettingsStore` расширять было нельзя: это хранилище **устройства**, оно
//! собирает путь через `directories` на native и пишет в OPFS на web. Про
//! открытый vault оно не знает и знать не должно — сцепление двух независимых
//! подсистем `settings::apply` избегает намеренно.
//!
//! # Слой, а не копия
//!
//! Каждое поле — `Option`, и `None` означает «спросить устройство», а не
//! «значение по умолчанию» (FR-030). Разница выразима в системе типов, а не в
//! соглашении, — тот же приём, что у `SettingsChange`, где `None` означает
//! «не трогать».
//!
//! Список известных vault и текущий vault здесь отсутствуют **физически**
//! (FR-028): без них неоткуда узнать, какую папку открывать, поэтому положить
//! их внутрь одной из папок нельзя даже по ошибке.

use super::backend::{ReadError, VaultBackend, WriteError};
use super::sections::SectionMark;
use super::service_dir;
use crate::boards::placement::AssetsPlacement;
use crate::settings::keybindings::{CommandId, KeybindingEntry, Shortcut, ShortcutProblem};
use crate::settings::{ModePreference, Theme};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::PathBuf;

/// Настройки, принадлежащие vault (data-model.md «VaultOverrides»).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct VaultOverrides {
    pub assets_placement: Option<AssetsPlacement>,
    /// Не `Option`: пустой список и «не задано» здесь неразличимы по смыслу.
    pub sections: Vec<SectionMark>,
    pub theme: Option<Theme>,
    pub editor_font_size: Option<u8>,
    pub editor_mode_preference: Option<ModePreference>,
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string | undefined"))]
    pub editor_config_path: Option<PathBuf>,

    /// Ходить ли в сеть за недостающими внешними шаблонами (фича 007, FR-009).
    ///
    /// `None` — значение по умолчанию, то есть **включено**: пользователь
    /// выбрал автоматическую загрузку основным поведением, а выключатель
    /// существует, чтобы это прекратить.
    ///
    /// Принадлежит папке заметок, как и прочие значения её уровня: шаблоны
    /// нужны конкретному набору заметок, а не устройству.
    pub auto_download_templates: Option<bool>,
    /// Адреса, указанные пользователем для конкретных шаблонов (FR-004).
    ///
    /// Ключ — та же строка спецификации, что видит пользователь:
    /// `@preview/codly:1.3.0`. Именно для шаблона, а не как глобальная замена
    /// реестра: подмена реестра целиком превратила бы любую заметку в источник
    /// неожиданного кода.
    pub template_sources: Vec<TemplateSourceEntry>,

    /// Сочетания клавиш команд приложения.
    ///
    /// Не `Option`: пустой список — это «ничего не переназначено», и отличать
    /// его от «не задано» здесь не от чего. Команда, которой в списке нет,
    /// работает на умолчании ядра; команда с `shortcut: None` — со снятым
    /// сочетанием, и эти два состояния различны (см.
    /// [`crate::settings::keybindings::KeybindingEntry`]).
    ///
    /// Принадлежит папке заметок, а не устройству: панель настроек существует
    /// пока только в web-сборке, а до настроек уровня vault добираются все три
    /// оболочки через один и тот же `VaultBackend`.
    pub keybindings: Vec<KeybindingEntry>,
}

/// Адрес архива, указанный пользователем для одного шаблона (фича 007, FR-004).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TemplateSourceEntry {
    /// Спецификация шаблона слово в слово, как в тексте подключения.
    pub spec: String,
    pub url: String,
}

impl VaultOverrides {
    /// Политика с учётом умолчания (FR-003).
    pub fn placement(&self) -> AssetsPlacement {
        self.assets_placement.unwrap_or_default()
    }

    /// Правила загрузки шаблонов для сборки (фича 007).
    ///
    /// Собирается здесь, а не в предпросмотре: «включена ли автозагрузка» и
    /// «какой адрес у этого шаблона» — значения настроек, и знать их обязано
    /// то место, которое их хранит.
    pub fn template_policy(&self) -> crate::preview::compile::TemplatePolicy {
        crate::preview::compile::TemplatePolicy {
            // Умолчание — включено (FR-009, Assumptions спеки 007).
            auto_download: self.auto_download_templates.unwrap_or(true),
            sources: self
                .template_sources
                .iter()
                .map(|entry| (entry.spec.clone(), entry.url.clone()))
                .collect(),
        }
    }
}

/// Почему настройки уровня vault не прочитаны или не записаны.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum VaultSettingsError {
    /// Файл есть, но не разбирается. Перезаписывать его нельзя (FR-034) —
    /// то же правило, что для `settings.json` в фиче 004.
    #[error("файл настроек vault не разбирается")]
    Unreadable,
    /// Vault недоступен на запись: служебное место не создаётся (FR-025).
    #[error("vault недоступен на запись")]
    NotWritable,
    #[error("не удалось прочитать или записать настройки vault")]
    Io,
    /// Адрес шаблона задан негодно: спецификация не разбирается или адрес не
    /// `http(s)://…` (фича 007, FR-004). Файл при этом не тронут — ни эта
    /// запись, ни остальные.
    #[error("адрес для шаблона «{spec}» задан негодно")]
    InvalidTemplateSource { spec: String },
}

/// Читает настройки уровня vault.
///
/// Отсутствие файла — **штатное состояние**, а не ошибка: vault, в котором
/// пользователь ничего не настраивал, отдаёт пустой набор перекрытий, и всё
/// берётся с устройства (FR-023).
pub async fn read_vault_settings<B: VaultBackend>(
    backend: &B,
) -> Result<VaultOverrides, VaultSettingsError> {
    let path = service_dir::vault_settings_path();
    match backend.read(&path).await {
        Ok(text) => parse(&text).ok_or(VaultSettingsError::Unreadable),
        Err(ReadError::NotFound) => Ok(VaultOverrides::default()),
        Err(ReadError::TooLarge) => Err(VaultSettingsError::Unreadable),
        Err(ReadError::Io) => Err(VaultSettingsError::Io),
    }
}

/// Годится ли пара «шаблон → адрес» (фича 007, FR-004).
///
/// Спецификация обязана разбираться тем же разбором, которым сборка ищет
/// источник: иначе опечатка в имени молча оставила бы шаблон на реестре, а
/// пользователь считал бы, что указал свой адрес. Адрес — только `http(s)`:
/// прочие схемы загрузчик не обслуживает, и принять их значило бы отложить
/// отказ до первой сборки.
pub fn is_valid_template_source(entry: &TemplateSourceEntry) -> bool {
    let url = entry.url.trim();
    let has_host = |rest: &str| !rest.is_empty() && !rest.starts_with('/');
    crate::preview::packages::parse_spec(entry.spec.trim()).is_some()
        && (url.strip_prefix("https://").is_some_and(has_host)
            || url.strip_prefix("http://").is_some_and(has_host))
        && !url.chars().any(char::is_whitespace)
}

/// Записывает настройки уровня vault поверх существующего файла.
///
/// Ключи, дописанные пользователем руками, переживают запись из панели — тот же
/// приём и та же причина, что в `settings::format`: файл принадлежит
/// пользователю.
pub async fn write_vault_settings<B: VaultBackend>(
    backend: &mut B,
    overrides: &VaultOverrides,
) -> Result<(), VaultSettingsError> {
    // Проверка до первой записи: негодный адрес не должен оставить после
    // себя ни служебной папки, ни полупереписанного файла.
    if let Some(bad) = overrides
        .template_sources
        .iter()
        .find(|entry| !is_valid_template_source(entry))
    {
        return Err(VaultSettingsError::InvalidTemplateSource {
            spec: bad.spec.clone(),
        });
    }

    ensure_service_dir(backend).await?;

    let path = service_dir::vault_settings_path();
    // Файла ещё нет — писать поверх нечего, основой становится пустая строка.
    let previous = backend.read(&path).await.unwrap_or_default();
    let text = render(&previous, overrides);

    backend
        .write(&path, &text, None)
        .await
        .map_err(to_settings_error)
}

/// Что сказать пользователю о служебном месте сразу после открытия vault.
///
/// `there_was_something_to_lose` — открывали ли **эту** папку раньше, то есть
/// могли ли в ней вообще быть настройки уровня vault. Приходит снаружи: сам
/// backend этого не знает, а придумывать ответ вместо вызывающей стороны
/// нельзя — ложное сообщение об утрате хуже его отсутствия (FR-024).
///
/// Три исхода, и порядок проверок значим:
///
/// 1. Vault недоступен на запись — служебное место не создано, менять
///    настройки некуда (FR-025). Более сильный факт, чем отсутствие папки: её
///    там и не могло появиться.
/// 2. Терять было что, а служебного места нет — его удалили вместе с
///    настройками и локальными шаблонами (FR-024).
/// 3. Всё остальное, включая первое открытие новой папки, — сказать нечего.
///
/// # Кто передаёт первый аргумент
///
/// Спека называет источником `Settings::known_vaults` этого устройства.
/// **Эта часть пока не подключена**: список известных vault принадлежит панели
/// настроек, а открытие папки идёт мимо неё, и протаскивать туда хранилище
/// устройства ради одного предупреждения — отдельное решение, которое стоит
/// принимать осознанно, а не походя. Второй исход поэтому в интерфейсе ещё не
/// наступает; первый (`VaultSettingsReadOnly`) работает и подключён.
pub fn service_dir_status<B: VaultBackend>(
    backend: &B,
    there_was_something_to_lose: bool,
) -> Option<super::events::VaultEvent> {
    use super::events::VaultEvent;
    if backend.access_state() != super::AccessState::ReadWrite {
        return Some(VaultEvent::VaultSettingsReadOnly);
    }
    if there_was_something_to_lose && !backend.service_dir_existed_at_open() {
        return Some(VaultEvent::VaultSettingsLost);
    }
    None
}

/// Помечает папку как раздел или снимает пометку (FR-013).
///
/// Живёт в ядре, а не в двух биндингах: правило «что делает пометка» — это
/// бизнес-логика (принцип I), и две её копии на границе native и web разошлись
/// бы молча и по-разному.
///
/// Снятие пометки **ничего не двигает на диске** (FR-017): уже созданные файлы
/// вставок остаются там, где лежат, меняется только точка, куда попадут новые.
///
/// Возвращает обновлённый набор настроек — UI обязан показать новое состояние
/// дерева, и второй запрос за тем, что здесь уже посчитано, был бы лишним
/// кругом через границу.
pub async fn set_section<B: VaultBackend>(
    backend: &mut B,
    folder: &std::path::Path,
    marked: bool,
) -> Result<VaultOverrides, VaultSettingsError> {
    let Some(mark) = SectionMark::new(folder) else {
        // Путь за пределы vault пометкой стать не может; молча согласиться
        // значило бы записать в файл то, чем нельзя пользоваться.
        return Err(VaultSettingsError::Io);
    };

    let mut overrides = read_vault_settings(backend).await?;
    let position = overrides
        .sections
        .iter()
        .position(|existing| existing.path == mark.path);

    match (marked, position) {
        // Повторная пометка уже помеченной папки — не ошибка и не дубль.
        (true, Some(_)) | (false, None) => return Ok(overrides),
        (true, None) => overrides.sections.push(mark),
        (false, Some(at)) => {
            overrides.sections.remove(at);
        }
    }
    // Порядок в файле детерминирован: иначе каждая пометка переставляла бы
    // строки в файле, который пользователь может читать глазами.
    overrides.sections.sort();

    write_vault_settings(backend, &overrides).await?;
    Ok(overrides)
}

/// Почему сочетание не назначено.
///
/// Отдельно от [`VaultSettingsError`]: негодное сочетание — это проверка
/// значения, не зависящая от состояния хранилища, и путать её с отказом диска
/// нельзя. Пользователю про них говорят разное.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind", content = "reason")]
pub enum SetKeybindingError {
    #[error("сочетание не годится: {0}")]
    Invalid(ShortcutProblem),
    #[error("настройки vault не записаны: {0}")]
    Storage(VaultSettingsError),
}

/// Назначает команде сочетание либо снимает его.
///
/// `shortcut: None` — снять: команда останется в палитре, но клавишами
/// вызываться не будет. Это не то же, что «вернуть умолчание», — для возврата
/// служит [`reset_keybinding`].
///
/// Живёт в ядре, а не в двух биндингах, по той же причине, что `set_section`:
/// «какое сочетание допустимо» — правило, и две его копии на границе native и
/// web разошлись бы молча.
///
/// Конфликт с другой командой отказом **не является**: он показывается в
/// разобранном виде ([`crate::settings::keybindings::ResolvedBinding`]), а
/// разрешает его пользователь. Запрет заставил бы расставлять сочетания в
/// определённом порядке — поменять местами два сочетания стало бы невозможно.
///
/// Возвращает обновлённый набор настроек — по той же причине, что `set_section`:
/// UI обязан показать новое состояние, и второй запрос за уже посчитанным был
/// бы лишним кругом через границу.
pub async fn set_keybinding<B: VaultBackend>(
    backend: &mut B,
    command: CommandId,
    shortcut: Option<Shortcut>,
) -> Result<VaultOverrides, SetKeybindingError> {
    if let Some(candidate) = &shortcut {
        if let Some(problem) = candidate.problem() {
            return Err(SetKeybindingError::Invalid(problem));
        }
    }

    let mut overrides = read_vault_settings(backend)
        .await
        .map_err(SetKeybindingError::Storage)?;
    // Нормализация здесь, а не у вызывающей стороны: хранимое значение обязано
    // быть каноничным независимо от того, как его собрали.
    let shortcut = shortcut.map(|value| Shortcut::normalize(value.mod_key, value.shift, value.alt, &value.key));

    match overrides
        .keybindings
        .iter_mut()
        .find(|entry| entry.command == command)
    {
        Some(existing) => existing.shortcut = shortcut,
        None => overrides
            .keybindings
            .push(KeybindingEntry { command, shortcut }),
    }
    sort_keybindings(&mut overrides);

    write_vault_settings(backend, &overrides)
        .await
        .map_err(SetKeybindingError::Storage)?;
    Ok(overrides)
}

/// Возвращает команду к умолчанию ядра — убирает запись целиком.
///
/// Отдельно от `set_keybinding(None)`: там `None` означает «снять сочетание»,
/// здесь — «забыть, что его меняли». Одной функцией эти два действия не
/// выражаются.
pub async fn reset_keybinding<B: VaultBackend>(
    backend: &mut B,
    command: CommandId,
) -> Result<VaultOverrides, VaultSettingsError> {
    let mut overrides = read_vault_settings(backend).await?;
    let before = overrides.keybindings.len();
    overrides.keybindings.retain(|entry| entry.command != command);
    // Ничего не изменилось — не трогаем файл: лишняя запись на каждый клик это
    // лишний повод его испортить (тот же довод, что в `move_sections`).
    if overrides.keybindings.len() == before {
        return Ok(overrides);
    }
    sort_keybindings(&mut overrides);
    write_vault_settings(backend, &overrides).await?;
    Ok(overrides)
}

/// Порядок записей в файле детерминирован: иначе правка одного сочетания
/// переставляла бы строки в файле, который пользователь может читать глазами.
fn sort_keybindings(overrides: &mut VaultOverrides) {
    overrides
        .keybindings
        .sort_by_key(|entry| entry.command.as_str());
}

/// Переносит пометки вслед за переименованием папки средствами приложения
/// (FR-018) и сохраняет их, если что-то изменилось.
///
/// Ничего не делает, если ни одна пометка не затронута: лишняя запись файла
/// на каждое переименование заметки — это лишний повод его испортить.
pub async fn move_sections<B: VaultBackend>(
    backend: &mut B,
    from: &std::path::Path,
    to: &std::path::Path,
) -> Result<(), VaultSettingsError> {
    let mut overrides = read_vault_settings(backend).await?;
    if !super::sections::rename_section(&mut overrides.sections, from, to) {
        return Ok(());
    }
    overrides.sections.sort();
    write_vault_settings(backend, &overrides).await
}

/// Создаёт служебную папку, если её ещё нет.
///
/// Вызывается при открытии vault, доступного на запись, — одинаково на обоих
/// таргетах, включая web, где файла кэша нет (research.md §12). Иначе одно и то
/// же действие пользователя давало бы разный наблюдаемый результат на двух
/// клиентах, а расхождение в том, что пользователь видит в своей папке, — не
/// тот случай, где принцип II можно считать соблюдённым «по форме API».
pub async fn ensure_service_dir<B: VaultBackend>(
    backend: &mut B,
) -> Result<(), VaultSettingsError> {
    backend
        .ensure_folder(&service_dir::service_dir())
        .await
        .map_err(to_settings_error)
}

/// Откладывает испорченный файл рядом, не перезаписывая его (FR-034).
///
/// `utc_millis` приходит снаружи, а не берётся здесь: часов в ядре нет ни на
/// одном таргете, и заводить их ради имени файла значило бы завести
/// платформенный шов на ровном месте.
pub async fn set_aside_invalid<B: VaultBackend>(
    backend: &mut B,
    utc_millis: i64,
) -> Result<PathBuf, VaultSettingsError> {
    let path = service_dir::vault_settings_path();
    let aside = path.with_file_name(format!(
        "{}.invalid-{utc_millis}",
        service_dir::VAULT_SETTINGS_FILE_NAME
    ));
    backend
        .rename(&path, &aside)
        .await
        .map_err(|_| VaultSettingsError::Io)?;
    Ok(aside)
}

fn to_settings_error(error: WriteError) -> VaultSettingsError {
    match error {
        WriteError::AccessDenied => VaultSettingsError::NotWritable,
        WriteError::Conflict | WriteError::Io => VaultSettingsError::Io,
    }
}

// --- Формат файла -------------------------------------------------------

const ASSETS_TABLE: &str = "assets";
const SETTINGS_TABLE: &str = "settings";
const TEMPLATES_TABLE: &str = "templates";
const KEYBINDINGS_TABLE: &str = "keybindings";

/// Форма файла для serde. Отдельно от [`VaultOverrides`], потому что на диске
/// значения разложены по двум таблицам, а в API нужен один плоский набор.
#[derive(Debug, Default, Deserialize)]
struct FileShape {
    #[serde(default)]
    assets: AssetsTable,
    #[serde(default)]
    settings: SettingsTable,
    #[serde(default)]
    templates: TemplatesTable,
    /// Имя команды → текстовая форма сочетания. `null` — сочетание снято.
    ///
    /// Объект, а не массив: файл читают глазами, и `"search": "Mod+Shift+F"`
    /// понятнее списка записей с двумя полями.
    #[serde(default)]
    keybindings: std::collections::BTreeMap<String, Option<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct TemplatesTable {
    #[serde(default)]
    auto_download: Option<bool>,
    /// Спецификация шаблона → адрес архива.
    #[serde(default)]
    sources: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
struct AssetsTable {
    #[serde(default)]
    placement: Option<AssetsPlacement>,
    #[serde(default)]
    sections: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SettingsTable {
    #[serde(default)]
    theme: Option<Theme>,
    #[serde(default)]
    editor_font_size: Option<u8>,
    #[serde(default)]
    editor_mode_preference: Option<ModePreference>,
    #[serde(default)]
    editor_config_path: Option<PathBuf>,
}

/// Разбирает содержимое файла. `None` — файл не разбирается.
///
/// Неизвестные ключи ошибкой не считаются: файл принадлежит пользователю, и
/// отказ читать его из-за лишней строки был бы враждебен — то же решение, что
/// в `settings::format::parse`.
///
/// Непригодная пометка раздела (абсолютный путь, выход через `..`)
/// **отбрасывается молча**, а не роняет разбор: одна испорченная строка не
/// повод объявить нечитаемым весь файл и лишить пользователя остальных
/// настроек.
pub fn parse(text: &str) -> Option<VaultOverrides> {
    // Пустой файл — это пустой набор перекрытий, а не поломка: таким остаётся
    // только что созданный файл после оборванной записи, и объявлять его
    // нечитаемым значило бы запретить запись в него навсегда (FR-034).
    if text.trim().is_empty() {
        return Some(VaultOverrides::default());
    }
    let shape: FileShape = serde_json::from_str(text).ok()?;
    Some(shape.into_overrides())
}

impl FileShape {
    /// Приводит форму файла к набору перекрытий.
    ///
    /// Отдельно от разбора: [`FileShape`] описывает раскладку по объектам на
    /// диске, а наружу нужен один плоский набор, и отбор непригодных значений —
    /// правило, а не следствие формата.
    fn into_overrides(self) -> VaultOverrides {
        VaultOverrides {
            assets_placement: self.assets.placement,
            sections: self
                .assets
                .sections
                .iter()
                .filter_map(|path| SectionMark::new(std::path::Path::new(path)))
                .collect(),
            theme: self.settings.theme,
            // Диапазон не имеет права разойтись с проверкой на устройстве
            // (принцип II): значение вне него — как если бы его не задали.
            editor_font_size: self
                .settings
                .editor_font_size
                .filter(|size| crate::settings::is_valid_font_size(*size)),
            editor_mode_preference: self.settings.editor_mode_preference,
            editor_config_path: self.settings.editor_config_path,
            auto_download_templates: self.templates.auto_download,
            template_sources: self
                .templates
                .sources
                .into_iter()
                .map(|(spec, url)| TemplateSourceEntry { spec, url })
                .collect(),
            keybindings: self
                .keybindings
                .into_iter()
                .filter_map(|(name, text)| {
                    // Незнакомая команда и неразбираемое сочетание
                    // **отбрасываются молча** — как непригодная пометка раздела
                    // выше. Одна испорченная строка не повод объявить файл
                    // нечитаемым и лишить пользователя остальных настроек;
                    // отброшенная команда просто остаётся на умолчании.
                    let command = CommandId::parse(&name)?;
                    let shortcut = match text {
                        // `null` — сочетание снято пользователем. Это значение,
                        // а не отсутствие записи.
                        None => None,
                        Some(text) => {
                            let parsed = Shortcut::parse(&text)?;
                            // Недопустимое сочетание — как если бы его не
                            // задали: правило не имеет права разойтись с тем,
                            // что проверяет панель (принцип II).
                            if parsed.problem().is_some() {
                                return None;
                            }
                            Some(parsed)
                        }
                    };
                    Some(KeybindingEntry { command, shortcut })
                })
                .collect(),
        }
    }
}

/// Записывает значения поверх существующего текста.
///
/// Ключи, которых приложение не знает, **переносятся как есть**: разбор идёт
/// через `serde_json::Value`, а не через [`FileShape`], и всё лишнее остаётся в
/// объекте нетронутым (FR-013 фичи 004, то же правило).
pub fn render(previous: &str, overrides: &VaultOverrides) -> String {
    let mut document = match serde_json::from_str::<Value>(previous) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    };

    set_or_remove(
        &mut document,
        ASSETS_TABLE,
        "placement",
        overrides
            .assets_placement
            .map(|placement| Value::from(placement_str(placement))),
    );
    set_or_remove(
        &mut document,
        ASSETS_TABLE,
        "sections",
        (!overrides.sections.is_empty()).then(|| {
            Value::Array(
                overrides
                    .sections
                    .iter()
                    .map(|section| {
                        Value::from(section.path.to_string_lossy().replace('\\', "/"))
                    })
                    .collect(),
            )
        }),
    );

    set_or_remove(
        &mut document,
        SETTINGS_TABLE,
        "theme",
        overrides.theme.map(|theme| Value::from(theme_str(theme))),
    );
    set_or_remove(
        &mut document,
        SETTINGS_TABLE,
        "editor_font_size",
        overrides.editor_font_size.map(Value::from),
    );
    set_or_remove(
        &mut document,
        SETTINGS_TABLE,
        "editor_mode_preference",
        overrides
            .editor_mode_preference
            .map(|mode| Value::from(mode_str(mode))),
    );
    set_or_remove(
        &mut document,
        SETTINGS_TABLE,
        "editor_config_path",
        overrides
            .editor_config_path
            .as_ref()
            .map(|path| Value::from(path.to_string_lossy().into_owned())),
    );

    set_or_remove(
        &mut document,
        TEMPLATES_TABLE,
        "auto_download",
        overrides.auto_download_templates.map(Value::from),
    );
    set_or_remove(
        &mut document,
        TEMPLATES_TABLE,
        "sources",
        (!overrides.template_sources.is_empty()).then(|| {
            // Порядок детерминирован сам: ключи `serde_json::Map` отсортированы
            // (`preserve_order` в проекте включать нельзя — см.
            // `settings::format`). Отдельная сортировка поэтому не нужна.
            let mut sources = Map::new();
            for entry in &overrides.template_sources {
                sources.insert(entry.spec.clone(), Value::from(entry.url.clone()));
            }
            Value::Object(sources)
        }),
    );

    // Таблица сочетаний переписывается целиком, а не по ключу: запись здесь
    // всегда содержит полный список назначенного, и ключ, которого в нём нет,
    // означает возврат команды к умолчанию. Оставлять его в файле значило бы
    // сохранить назначение, которое пользователь только что снял.
    if overrides.keybindings.is_empty() {
        document.remove(KEYBINDINGS_TABLE);
    } else {
        let mut table = Map::new();
        for entry in &overrides.keybindings {
            table.insert(
                entry.command.as_str().to_string(),
                match &entry.shortcut {
                    Some(shortcut) => Value::from(shortcut.to_text()),
                    // Снятое сочетание пишется как `null`: отсутствие ключа уже
                    // занято смыслом «умолчание».
                    None => Value::Null,
                },
            );
        }
        document.insert(KEYBINDINGS_TABLE.to_string(), Value::Object(table));
    }

    prune_empty_object(&mut document, ASSETS_TABLE);
    prune_empty_object(&mut document, SETTINGS_TABLE);
    prune_empty_object(&mut document, TEMPLATES_TABLE);

    let mut text = serde_json::to_string_pretty(&Value::Object(document))
        .unwrap_or_else(|_| "{}".to_string());
    text.push('\n');
    text
}

/// Объект верхнего уровня по имени, создавая его при необходимости.
fn object_mut<'a>(document: &'a mut Map<String, Value>, name: &str) -> &'a mut Map<String, Value> {
    if !document.get(name).is_some_and(Value::is_object) {
        document.insert(name.to_string(), Value::Object(Map::new()));
    }
    document
        .get_mut(name)
        .and_then(Value::as_object_mut)
        .expect("объект только что создан")
}

/// Пишет значение внутрь объекта либо убирает ключ, если значения нет.
fn set_or_remove(
    document: &mut Map<String, Value>,
    table: &str,
    key: &str,
    new_value: Option<Value>,
) {
    match new_value {
        Some(value) => {
            object_mut(document, table).insert(key.to_string(), value);
        }
        None => remove_key(document, table, key),
    }
}

fn remove_key(document: &mut Map<String, Value>, table: &str, key: &str) {
    if let Some(object) = document.get_mut(table).and_then(Value::as_object_mut) {
        object.remove(key);
    }
}

/// Убирает объект, из которого всё вычистили: пустой `"settings": {}` в файле
/// пользователя — мусор, оставленный приложением.
fn prune_empty_object(document: &mut Map<String, Value>, name: &str) {
    let empty = document
        .get(name)
        .and_then(Value::as_object)
        .is_some_and(Map::is_empty);
    if empty {
        document.remove(name);
    }
}

fn placement_str(placement: AssetsPlacement) -> &'static str {
    match placement {
        AssetsPlacement::NextToNote => "next-to-note",
        AssetsPlacement::VaultRoot => "vault-root",
        AssetsPlacement::SectionRoot => "section-root",
    }
}

fn theme_str(theme: Theme) -> &'static str {
    match theme {
        Theme::Light => "light",
        Theme::Dark => "dark",
    }
}

fn mode_str(preference: ModePreference) -> &'static str {
    match preference {
        ModePreference::FullBackend => "fullBackend",
        ModePreference::SimplifiedEmulation => "simplifiedEmulation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn sample() -> VaultOverrides {
        VaultOverrides {
            assets_placement: Some(AssetsPlacement::SectionRoot),
            sections: vec![
                SectionMark::new(Path::new("Работа")).unwrap(),
                SectionMark::new(Path::new("Личное/Дневник")).unwrap(),
            ],
            theme: Some(Theme::Dark),
            editor_font_size: Some(16),
            editor_mode_preference: Some(ModePreference::FullBackend),
            editor_config_path: Some("/home/user/.config/nvim".into()),
            // Фича 007: загрузка внешних шаблонов.
            auto_download_templates: Some(false),
            template_sources: vec![TemplateSourceEntry {
                spec: "@preview/codly:1.3.0".to_string(),
                url: "https://example.org/codly.tar.gz".to_string(),
            }],
            keybindings: vec![
                KeybindingEntry {
                    command: CommandId::CommandPalette,
                    shortcut: Some(Shortcut::normalize(true, false, true, "p")),
                },
                KeybindingEntry {
                    command: CommandId::Search,
                    shortcut: None,
                },
            ],
        }
    }

    #[test]
    fn a_keybinding_survives_the_round_trip() {
        let text = render("", &sample());
        let parsed = parse(&text).unwrap();
        assert_eq!(parsed.keybindings, sample().keybindings);
        // В файле сочетание лежит словами, а не структурой: его читают глазами.
        assert!(text.contains("\"command-palette\": \"Mod+Alt+P\""), "{text}");
    }

    #[test]
    fn a_removed_shortcut_is_null_not_a_missing_key() {
        // Отсутствие ключа уже занято смыслом «умолчание», поэтому снятое
        // сочетание обязано быть записано явно.
        let text = render("", &sample());
        assert!(text.contains("\"search\": null"), "{text}");

        let parsed = parse(&text).unwrap();
        let search = parsed
            .keybindings
            .iter()
            .find(|entry| entry.command == CommandId::Search)
            .expect("снятое сочетание — это запись, а не её отсутствие");
        assert_eq!(search.shortcut, None);
    }

    #[test]
    fn an_unusable_keybinding_is_dropped_without_failing_the_file() {
        // Та же политика, что у непригодной пометки раздела: одна испорченная
        // строка не лишает пользователя остальных настроек.
        let text = r#"{
          "keybindings": {
            "command-palette": "Mod+P",
            "нет-такой-команды": "Mod+Q",
            "search": "не сочетание вовсе",
            "templates": "F",
            "new-file": "Mod+Escape"
          },
          "settings": {"theme": "dark"}
        }"#;
        let overrides = parse(text).expect("лишняя строка не делает файл нечитаемым");
        assert_eq!(overrides.theme, Some(Theme::Dark));
        // Уцелела ровно годная запись: незнакомая команда, неразбираемая
        // строка, сочетание без модификатора и занятая клавиша отброшены.
        assert_eq!(overrides.keybindings.len(), 1);
        assert_eq!(overrides.keybindings[0].command, CommandId::CommandPalette);
    }

    #[test]
    fn clearing_every_keybinding_removes_the_table() {
        let previous = render("", &sample());
        let cleared = VaultOverrides {
            keybindings: Vec::new(),
            ..sample()
        };
        let rewritten = render(&previous, &cleared);
        assert!(
            !rewritten.contains("keybindings"),
            "пустая таблица — мусор, оставленный приложением: {rewritten}"
        );
        assert_eq!(parse(&rewritten).unwrap(), cleared);
    }

    #[test]
    fn auto_download_is_on_unless_the_file_says_otherwise() {
        // FR-009 фичи 007: выключатель существует, чтобы прекратить
        // автозагрузку, а не чтобы включить её.
        assert!(VaultOverrides::default().template_policy().auto_download);

        let off = parse(r#"{"templates": {"auto_download": false}}"#).unwrap();
        assert!(!off.template_policy().auto_download);
    }

    #[test]
    fn a_template_source_survives_the_round_trip() {
        // FR-004: указанный адрес принадлежит конкретному шаблону.
        let text = render("", &sample());
        let parsed = parse(&text).unwrap();
        assert_eq!(parsed.template_sources.len(), 1);
        assert_eq!(parsed.template_sources[0].spec, "@preview/codly:1.3.0");

        let policy = parsed.template_policy();
        assert_eq!(
            policy.sources,
            vec![(
                "@preview/codly:1.3.0".to_string(),
                "https://example.org/codly.tar.gz".to_string()
            )]
        );
    }

    #[test]
    fn round_trip_preserves_every_value() {
        let text = render("", &sample());
        assert_eq!(parse(&text).unwrap(), sample());
    }

    #[test]
    fn an_absent_file_means_ask_the_device() {
        // FR-030: `None` — это «спросить устройство», а не значение по
        // умолчанию.
        let empty = VaultOverrides::default();
        assert_eq!(parse("").unwrap(), empty);
        assert_eq!(empty.theme, None);
        assert_eq!(empty.editor_font_size, None);
        // FR-003: при этом политика по умолчанию — «рядом с заметкой».
        assert_eq!(empty.placement(), AssetsPlacement::NextToNote);
    }

    #[test]
    fn what_the_user_wrote_by_hand_survives_a_write() {
        // Комментариев в JSON нет, и притворяться иначе нельзя. Уцелевшая
        // половина прежнего требования: дописанное руками не выбрасывается.
        let authored = r#"{
  "settings": {"editor_font_size": 18, "theme": "dark"},
  "my_future_option": {"nested": true}
}"#;
        let mut overrides = parse(authored).unwrap();
        overrides.editor_font_size = Some(19);
        let rewritten = render(authored, &overrides);

        assert!(rewritten.contains("my_future_option"));
        assert!(rewritten.contains("nested"));
        assert_eq!(parse(&rewritten).unwrap().editor_font_size, Some(19));
        assert_eq!(parse(&rewritten).unwrap().theme, Some(Theme::Dark));
    }

    #[test]
    fn the_key_order_is_stable_between_writes() {
        // Порядок задаёт приложение, но он обязан быть одинаковым от записи к
        // записи: иначе правка одной настройки давала бы diff на весь файл.
        let first = render("", &sample());
        let second = render(&first, &sample());
        assert_eq!(first, second);
    }

    #[test]
    fn unknown_keys_survive_and_are_not_an_error() {
        let text = r#"{"settings": {"theme": "dark"}, "my_future_option": 42}"#;
        let overrides = parse(text).expect("лишний ключ не делает файл нечитаемым");
        assert_eq!(overrides.theme, Some(Theme::Dark));
        assert!(render(text, &overrides).contains("my_future_option"));
    }

    #[test]
    fn broken_syntax_is_unreadable_rather_than_silently_empty() {
        // FR-034: испорченный файл обязан быть отличим от пустого, иначе он
        // был бы перезаписан значениями по умолчанию.
        assert!(parse("{ assets: ").is_none());
        // А пустой — именно пустой: он остаётся от оборванной записи, и
        // запрещать запись в него навсегда незачем.
        assert_eq!(parse("").unwrap(), VaultOverrides::default());
    }

    #[test]
    fn clearing_a_value_removes_its_key_and_its_empty_object() {
        let previous = render("", &sample());
        let cleared = VaultOverrides {
            assets_placement: Some(AssetsPlacement::VaultRoot),
            ..VaultOverrides::default()
        };
        let rewritten = render(&previous, &cleared);
        assert!(!rewritten.contains("theme"));
        assert!(!rewritten.contains("editor_config_path"));
        assert!(!rewritten.contains("sections"));
        assert!(
            !rewritten.contains("\"settings\""),
            "пустой объект — мусор, оставленный приложением: {rewritten}"
        );
        assert_eq!(parse(&rewritten).unwrap(), cleared);
    }

    #[test]
    fn an_unusable_section_is_dropped_without_failing_the_file() {
        // Одна испорченная строка не повод лишить пользователя остальных
        // настроек.
        let text =
            r#"{"assets": {"sections": ["Работа", "../снаружи"]}, "settings": {"theme": "dark"}}"#;
        let overrides = parse(text).unwrap();
        assert_eq!(overrides.sections.len(), 1);
        assert_eq!(overrides.sections[0].path, Path::new("Работа"));
    }

    #[test]
    fn a_font_size_outside_the_range_is_rejected_the_same_way_as_on_the_device() {
        // Принцип II: диапазон не имеет права разойтись между хранилищами.
        let text = r#"{"settings": {"editor_font_size": 99}}"#;
        assert_eq!(parse(text).unwrap().editor_font_size, None);
    }

    #[test]
    fn there_is_nowhere_to_put_the_vault_registry() {
        // FR-028: список известных vault здесь отсутствует физически, а не по
        // договорённости. Тест закрепляет, что ключ не подхватывается.
        let text = r#"{"settings": {"known_vaults": [{"id": "a", "location": "/x"}]}}"#;
        let overrides = parse(text).expect("лишний ключ не ошибка");
        assert_eq!(overrides, VaultOverrides::default());
    }

}
