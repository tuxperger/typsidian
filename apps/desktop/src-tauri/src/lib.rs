//! Оболочка рабочего стола (001 T112-T115).
//!
//! Здесь нет ни одного правила про vault: каждая команда — тонкая обёртка над
//! `typsidian_core` (принцип I). Проверить это можно по объёму тел: в них нет ни
//! одного `if`, который решал бы что-то за ядро, — только получение backend из
//! состояния и вызов.
//!
//! Чем эта оболочка отличается от web:
//! - backend — `NativeBackend` (`std::fs` + `notify`), а не WASM поверх File
//!   System Access API;
//! - события идут в окно **push**-ом: ядро сигналит `events_ready()`, задача
//!   ниже просыпается и пересылает `poll_events()` в окно (T114). Опроса по
//!   таймеру на native нет и не должно быть — обнаружение здесь событийное;
//! - папку выбирает нативный диалог ОС, а не `showDirectoryPicker` (T115).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use typsidian_core::boards::{
    self, Board, BoardEvent, CreateBoardError, Embed, EmbedError, EmbedPlan, EmbedRef,
    ReadBoardError, SnapshotWriteError, WriteBoardError,
};
use typsidian_core::stamp::FileStamp;
use typsidian_core::templates::{
    self, NameCheck, PreviewState, ReadTemplateError, SaveTemplateError, TemplateLibraryView,
};
use typsidian_core::vault::backend::{
    CreateError, DeleteError, OpenError, ReadError, RenameError, ResolveError, VaultBackend,
    WriteError,
};
use typsidian_core::vault::conflict::{DiffLine, EditConflict};
use typsidian_core::vault::index::Note;
use typsidian_core::vault::native::NativeBackend;
use typsidian_core::vault::events::VaultEvent;
use typsidian_core::vault::vault_settings::{self, VaultOverrides, VaultSettingsError};
use typsidian_core::vault::{classify, AccessState, FileKind};

/// Имя события, по которому окно получает пачку `VaultEvent`.
///
/// Одно на все варианты: разбор варианта — работа общего UI
/// (`applyVaultEvent`), и заводить по событию на вариант значило бы описать
/// размеченное объединение вторым способом.
const VAULT_EVENTS: &str = "vault://events";

/// Право этой сборки писать доски (фича 002, FR-035).
///
/// Выбирается таргетом, а не настройкой и не вызовом из окна: «мобильный
/// клиент открывает доску только на просмотр» — свойство сборки, и держать его
/// здесь значит, что мобильная оболочка получает ограничение самим фактом
/// сборки под Android/iOS, без своей копии этих команд. Проверка `target_os`, а
/// не `cfg(mobile)` от `tauri-build`: она не зависит от того, какие cfg отдаст
/// сборочный скрипт, и компилируется одинаково в любом окружении.
const BOARD_WRITE_PROFILE: boards::BoardWriteProfile =
    if cfg!(any(target_os = "android", target_os = "ios")) {
        boards::BoardWriteProfile::ViewOnly
    } else {
        boards::BoardWriteProfile::ReadWrite
    };

/// Открытый vault. `None` — пользователь ещё не выбрал папку.
///
/// `Mutex`, а не `RwLock`: половина операций `VaultBackend` требует `&mut self`
/// (запись, `begin_edit`, разрешение конфликта), поэтому разделяемого чтения
/// всё равно не получится.
#[derive(Default)]
pub struct Vault(Mutex<Option<NativeBackend>>);

/// Кэш превью заготовок (фича 003, research.md §4).
///
/// Отдельным состоянием окна, а не полем `Vault`: он не принадлежит бэкенду и
/// не обязан переживать его — производное восстановимое состояние, ключуемое
/// хэшем содержимого. Отдельный `Mutex` затем, чтобы построение превью не
/// становилось вторым поводом держать блокировку самого vault.
#[derive(Default)]
pub struct TemplatePreviews(Mutex<templates::PreviewCache>);

/// Vault ещё не открыт, а команда требует его.
///
/// Отдельный вариант, а не общий `Io`: UI обязан различать «папка не выбрана» и
/// «файл не прочитался» (FR-008).
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind")]
pub enum NoVault {
    NoVaultOpen,
}

type Answer<T, E> = Result<T, E>;

macro_rules! with_vault {
    ($state:expr, |$backend:ident| $body:expr) => {{
        let mut guard = $state.0.lock().await;
        let Some($backend) = guard.as_mut() else {
            return Err(NoVault::NoVaultOpen.into());
        };
        $body
    }};
}

/// Отказ команды: либо vault не открыт, либо причина от ядра.
///
/// Сериализуется как то же размеченное объединение, что и ошибки ядра
/// (`{"kind": …}`), поэтому общий UI разбирает отказы desktop и web одним кодом
/// (contracts/frontend-events.md).
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum Failure<E> {
    NoVault(NoVault),
    Core(E),
}

impl<E> From<NoVault> for Failure<E> {
    fn from(value: NoVault) -> Self {
        Failure::NoVault(value)
    }
}

// ── Открытие vault (FR-001, T115) ───────────────────────────────────────────

/// Открывает выбранную пользователем папку как vault.
///
/// Сам выбор папки делает нативный диалог на стороне окна
/// (`tauri-plugin-dialog`): это жест пользователя, и ядро его не инициирует —
/// то же правило, по которому `showDirectoryPicker` вызывается в UI (research.md
/// §1).
///
/// Пересылка событий запускается здесь же: подписываться на них до открытия
/// vault не на что.
#[tauri::command]
async fn open_vault(
    app: AppHandle,
    state: State<'_, Vault>,
    path: String,
) -> Answer<(), OpenError> {
    let backend = NativeBackend::open(Path::new(&path)).await?;
    let ready = backend.events_ready();
    *state.0.lock().await = Some(backend);
    spawn_event_forwarding(app, ready);
    Ok(())
}

/// Пересылка `VaultEvent` в окно (T114).
///
/// Ждёт сигнала ядра и отдаёт накопившееся окну одной пачкой. Пачкой, а не по
/// событию: массовое изменение снаружи (`git checkout`) обёрнуто в
/// `BulkChangeStarted`/`Settled`, и разрывать эту обёртку на отдельные emit'ы
/// значило бы отдать UI шторм уведомлений, который US3 AS5 запрещает.
fn spawn_event_forwarding(app: AppHandle, ready: Arc<tokio::sync::Notify>) {
    tauri::async_runtime::spawn(async move {
        loop {
            ready.notified().await;
            let Some(state) = app.try_state::<Vault>() else {
                return;
            };
            let events: Vec<VaultEvent> = {
                let mut guard = state.0.lock().await;
                match guard.as_mut() {
                    Some(backend) => backend.poll_events(),
                    None => return,
                }
            };
            if events.is_empty() {
                continue;
            }
            if app.emit(VAULT_EVENTS, events).is_err() {
                // Окна больше нет — пересылать некуда.
                return;
            }
        }
    });
}

// ── Дерево и файлы ──────────────────────────────────────────────────────────

#[tauri::command]
async fn list(state: State<'_, Vault>) -> Answer<Vec<Note>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend.list().await))
}

#[tauri::command]
async fn read(state: State<'_, Vault>, path: String) -> Answer<String, Failure<ReadError>> {
    with_vault!(state, |backend| backend
        .read(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn write(
    state: State<'_, Vault>,
    path: String,
    content: String,
    expected: Option<FileStamp>,
) -> Answer<(), Failure<WriteError>> {
    with_vault!(state, |backend| backend
        .write(Path::new(&path), &content, expected)
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn create(state: State<'_, Vault>, path: String) -> Answer<(), Failure<CreateError>> {
    with_vault!(state, |backend| backend
        .create(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn rename(
    state: State<'_, Vault>,
    from: String,
    to: String,
) -> Answer<(), Failure<RenameError>> {
    with_vault!(state, |backend| backend
        .rename(Path::new(&from), Path::new(&to))
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn delete(state: State<'_, Vault>, path: String) -> Answer<(), Failure<DeleteError>> {
    with_vault!(state, |backend| backend
        .delete(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn backlinks(state: State<'_, Vault>, path: String) -> Answer<Vec<Note>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend.backlinks(Path::new(&path)).await))
}

#[tauri::command]
async fn broken_links(state: State<'_, Vault>, path: String) -> Answer<Vec<PathBuf>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .broken_links(Path::new(&path))
        .await))
}

#[tauri::command]
async fn access_state(state: State<'_, Vault>) -> Answer<AccessState, Failure<()>> {
    with_vault!(state, |backend| Ok(backend.access_state()))
}

/// Расширенный поиск по vault (макет «Утилиты» § Поиск).
///
/// Обёртка, как и остальные команды: правила совпадения живут в ядре, здесь
/// только пересылка через границу процесса (принцип I).
#[tauri::command]
async fn search(
    state: State<'_, Vault>,
    query: typsidian_core::search::SearchQuery,
) -> Answer<typsidian_core::search::SearchResults, Failure<typsidian_core::search::SearchError>> {
    with_vault!(state, |backend| typsidian_core::search::search(
        backend, &query
    )
    .await
    .map_err(Failure::Core))
}

/// Путь папки, открытой как vault — окну для хлебных крошек в шапке.
///
/// Спрашивается у бэкенда, а не запоминается окном при выборе папки: тогда
/// «какой vault открыт» знали бы два места, и после переоткрытия они могли бы
/// разойтись.
#[tauri::command]
async fn root_path(state: State<'_, Vault>) -> Answer<String, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .root_path()
        .to_string_lossy()
        .into_owned()))
}

/// Право сборки писать доски — окну, чтобы объявить ограничение пользователю
/// (фича 002, FR-034).
///
/// Не требует открытого vault: профиль выбран таргетом при сборке, а не
/// состоянием папки, и окно спрашивает о нём до открытия чего-либо. Возвращает
/// ровно ту константу, которую применяет `write_board`, — сказанное
/// пользователю и применённое к записи не могут разойтись.
#[tauri::command]
fn board_write_profile() -> boards::BoardWriteProfile {
    BOARD_WRITE_PROFILE
}

/// Вид файла по пути. Не требует открытого vault: классификация идёт по пути,
/// без чтения содержимого (фича 002, research.md §3).
#[tauri::command]
fn file_kind(path: String) -> FileKind {
    classify(Path::new(&path))
}

// ── Несохранённые правки и конфликты ────────────────────────────────────────

#[tauri::command]
async fn begin_edit(
    state: State<'_, Vault>,
    path: String,
    buffer: String,
) -> Answer<(), Failure<()>> {
    with_vault!(state, |backend| {
        backend.begin_edit(Path::new(&path), buffer).await;
        Ok(())
    })
}

#[tauri::command]
async fn pending_conflict(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Option<EditConflict>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .pending_conflict(Path::new(&path))))
}

#[tauri::command]
async fn unresolved_edit_paths(state: State<'_, Vault>) -> Answer<Vec<PathBuf>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend.unresolved_edit_paths()))
}

#[tauri::command]
async fn conflict_diff(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Vec<DiffLine>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .conflict_diff(Path::new(&path))
        .await))
}

#[tauri::command]
async fn resolve_conflict_keep_local(
    state: State<'_, Vault>,
    path: String,
) -> Answer<(), Failure<ResolveError>> {
    with_vault!(state, |backend| backend
        .resolve_conflict_keep_local(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn resolve_conflict_accept_external(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Option<String>, Failure<ResolveError>> {
    with_vault!(state, |backend| backend
        .resolve_conflict_accept_external(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

// ── Доски и вставки (фича 002) ──────────────────────────────────────────────

/// Профиль записи зашит `ReadWrite` по той же причине, что и в web-биндинге:
/// рабочий стол рисует (FR-033 фичи 002), а просмотр без записи — свойство
/// мобильного клиента, у которого будет своя оболочка.
#[tauri::command]
async fn create_board(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Board, Failure<CreateBoardError>> {
    with_vault!(state, |backend| boards::create_board(backend, Path::new(&path))
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn read_board(
    state: State<'_, Vault>,
    path: String,
) -> Answer<String, Failure<ReadBoardError>> {
    with_vault!(state, |backend| {
        let content = boards::read_board(backend, Path::new(&path))
            .await
            .map_err(Failure::Core)?;
        String::from_utf8(content.as_bytes().to_vec())
            .map_err(|_| Failure::Core(ReadBoardError::Unreadable))
    })
}

#[tauri::command]
async fn write_board(
    state: State<'_, Vault>,
    path: String,
    content: String,
    expected: Option<FileStamp>,
) -> Answer<(), Failure<WriteBoardError>> {
    with_vault!(state, |backend| boards::write_board(
        backend,
        BOARD_WRITE_PROFILE,
        Path::new(&path),
        &boards::BoardContent::new(content.as_bytes().to_vec()),
        expected,
    )
    .await
    .map_err(Failure::Core))
}

#[tauri::command]
async fn write_snapshot(
    state: State<'_, Vault>,
    snapshot_path: String,
    svg: String,
    board_path: String,
    board_content: String,
) -> Answer<(), Failure<SnapshotWriteError>> {
    with_vault!(state, |backend| {
        let digest = boards::digest(board_content.as_bytes())
            .map_err(|_| Failure::Core(SnapshotWriteError::NotAnSvg))?;
        let metadata = boards::snapshot_metadata(Path::new(&board_path), &digest);
        boards::write_snapshot(backend, Path::new(&snapshot_path), &svg, &metadata)
            .await
            .map_err(Failure::Core)
    })
}

#[tauri::command]
async fn embeds_of(
    state: State<'_, Vault>,
    note_path: String,
    note_text: String,
) -> Answer<Vec<Embed>, Failure<()>> {
    with_vault!(state, |backend| Ok(boards::embeds_with_state(
        backend,
        Path::new(&note_path),
        &note_text
    )
    .await))
}

/// Разбор текста заметки. Vault не требуется: файлы не читаются.
#[tauri::command]
fn embeds_in_text(note_path: String, note_text: String) -> Vec<EmbedRef> {
    boards::embeds_of(Path::new(&note_path), &note_text)
}

#[tauri::command]
fn embeds_changed(
    note_path: String,
    note_text: String,
    changed: String,
) -> Option<BoardEvent> {
    boards::embeds_changed(
        Path::new(&note_path),
        &note_text,
        Path::new(&changed),
    )
}

/// Настройки vault приходят целиком, а не разобранными на политику и список
/// разделов: UI их уже прочитал, а решение «что из них влияет на путь»
/// принадлежит ядру (фича 006, принцип I).
#[tauri::command]
fn plan_embed(
    note_path: String,
    note_text: String,
    board_path: String,
    overrides: VaultOverrides,
) -> Answer<EmbedPlan, EmbedError> {
    boards::plan_embed(
        Path::new(&note_path),
        &note_text,
        Path::new(&board_path),
        overrides.placement(),
        &overrides.sections,
    )
}

// ── Настройки уровня vault и разделы (фича 006) ─────────────────────────────

/// Настройки уровня vault (FR-002, FR-015, FR-026).
#[tauri::command]
async fn vault_settings(
    state: State<'_, Vault>,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::read_vault_settings(backend)
            .await
            .map_err(Failure::Core)
    })
}

/// Записывает настройки уровня vault (FR-029).
#[tauri::command]
async fn set_vault_settings(
    state: State<'_, Vault>,
    overrides: VaultOverrides,
) -> Answer<(), Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::write_vault_settings(backend, &overrides)
            .await
            .map_err(Failure::Core)
    })
}

/// Помечает папку как корень раздела (FR-013).
#[tauri::command]
async fn mark_section(
    state: State<'_, Vault>,
    folder: String,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::set_section(backend, Path::new(&folder), true)
            .await
            .map_err(Failure::Core)
    })
}

/// Снимает пометку раздела (FR-013). Уже созданные файлы вставок при этом не
/// двигаются (FR-017).
#[tauri::command]
async fn unmark_section(
    state: State<'_, Vault>,
    folder: String,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::set_section(backend, Path::new(&folder), false)
            .await
            .map_err(Failure::Core)
    })
}

#[tauri::command]
fn embed_markup(plan: EmbedPlan) -> String {
    boards::embed_markup(&plan)
}

// ── Шаблоны и сниппеты (фича 003) ───────────────────────────────────────────
//
// Те же обёртки, что и всё выше: правила библиотеки живут в
// `typsidian_core::templates`, здесь только пересылка через границу процесса.

#[tauri::command]
async fn template_library(state: State<'_, Vault>) -> Answer<TemplateLibraryView, Failure<()>> {
    with_vault!(state, |backend| Ok(templates::library(backend).await))
}

#[tauri::command]
async fn read_template(
    state: State<'_, Vault>,
    path: String,
) -> Answer<String, Failure<ReadTemplateError>> {
    with_vault!(state, |backend| templates::read_template(
        backend,
        Path::new(&path)
    )
    .await
    .map_err(Failure::Core))
}

/// Превью одной заготовки.
///
/// Кэш — состояние оболочки, а не ядра: на web им владеет `WebBackend`, здесь —
/// эта команда. Разница вынужденная и безобидная: кэш производный,
/// восстановимый и авторитетной копией ничему не является (research.md §4).
/// Правило, по которому он ключуется хэшем содержимого, при этом одно и живёт в
/// ядре.
#[tauri::command]
async fn template_preview(
    state: State<'_, Vault>,
    cache: State<'_, TemplatePreviews>,
    template: templates::Template,
) -> Answer<PreviewState, Failure<()>> {
    let mut cache = cache.0.lock().await;
    with_vault!(state, |backend| Ok(templates::preview_of(
        backend,
        &templates::ExcerptOnlySource,
        &mut cache,
        &template
    )
    .await))
}

#[tauri::command]
async fn check_template_name(
    state: State<'_, Vault>,
    name: String,
) -> Answer<NameCheck, Failure<()>> {
    with_vault!(state, |backend| Ok(
        templates::check_name(backend, &name).await
    ))
}

#[tauri::command]
async fn save_as_template(
    state: State<'_, Vault>,
    name: String,
    content: String,
    confirm_overwrite: bool,
) -> Answer<templates::Template, Failure<SaveTemplateError>> {
    with_vault!(state, |backend| templates::save_as_template(
        backend,
        &name,
        &content,
        confirm_overwrite
    )
    .await
    .map_err(Failure::Core))
}

/// Какую правку совершить над текстом, чтобы вставить заготовку.
///
/// Открытого vault не требует: правило вставки текста от папки не зависит.
#[tauri::command]
fn plan_insertion(
    text_length: usize,
    point: templates::InsertionPoint,
    content: String,
) -> templates::InsertionEdit {
    templates::plan_insertion(text_length, &point, &content)
}

/// Затрагивает ли событие файла библиотеку заготовок.
#[tauri::command]
fn template_event(event: VaultEvent) -> Option<templates::TemplateEvent> {
    templates::library_event(&event)
}

/// Обход бага WebKitGTK на проприетарном драйвере NVIDIA.
///
/// Под Wayland с явной синхронизацией (`linux-drm-syncobj-v1`) DMABUF-рендерер
/// WebKit коммитит буфер, не выставив acquire-точку, и композитор отвечает
/// `wp_linux_drm_syncobj_surface_v1: "Missing acquire timeline"`. Окно не
/// доживает до первого кадра: GTK получает протокольную ошибку и завершает
/// процесс — воспроизводится 3 раза из 3 на NVIDIA + Hyprland.
///
/// Путь DMABUF выключается **только там, где он сломан**: на AMD и Intel он
/// исправен и нужен, чтобы полотно доски укладывалось в бюджет отклика ≤100 мс
/// (фича 002, SC-005). Значение, заданное пользователем снаружи, не трогается —
/// обход можно и отключить, и включить принудительно.
#[cfg(target_os = "linux")]
fn disable_dmabuf_renderer_on_nvidia() {
    const KEY: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
    if Path::new("/sys/module/nvidia_drm").exists() && std::env::var_os(KEY).is_none() {
        std::env::set_var(KEY, "1");
    }
}

#[cfg(not(target_os = "linux"))]
fn disable_dmabuf_renderer_on_nvidia() {}

/// Точка входа оболочки.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // До `Builder`, а не внутри него: переменную обязан увидеть WebKit при
    // инициализации, и менять окружение можно только пока процесс однопоточный.
    disable_dmabuf_renderer_on_nvidia();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Vault::default())
        .manage(TemplatePreviews::default())
        .invoke_handler(tauri::generate_handler![
            open_vault,
            list,
            read,
            write,
            create,
            rename,
            delete,
            backlinks,
            broken_links,
            access_state,
            search,
            file_kind,
            begin_edit,
            pending_conflict,
            unresolved_edit_paths,
            conflict_diff,
            resolve_conflict_keep_local,
            resolve_conflict_accept_external,
            root_path,
            board_write_profile,
            create_board,
            read_board,
            write_board,
            write_snapshot,
            embeds_of,
            embeds_in_text,
            embeds_changed,
            plan_embed,
            embed_markup,
            vault_settings,
            set_vault_settings,
            mark_section,
            unmark_section,
            template_library,
            read_template,
            template_preview,
            check_template_name,
            save_as_template,
            plan_insertion,
            template_event,
        ])
        .run(tauri::generate_context!())
        .expect("оболочка Typsidian должна запуститься");
}
