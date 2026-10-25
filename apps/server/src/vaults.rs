//! Vault аккаунта: папки `<данные>/users/<id>/<имя>/` (research.md §2, §4) и открытые
//! `Host` над ними.
//!
//! Путь к vault строит только сервер — из id аккаунта сессии и проверенного имени;
//! клиент путей файловой системы не передаёт. Чужой vault и несуществующий отвечают
//! одинаково (FR-004).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use axum::extract::{Path as UrlPath, State};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch, Mutex, Notify, OnceCell};
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::native::NativeBackend;
use typsidian_host::Host;

use crate::auth::Authed;
use crate::error::{ApiError, ApiResult};
use crate::Shared;

/// Имя vault — имя его папки: без разделителей путей, управляющих знаков, `..` и
/// ведущей точки (служебные папки), не длиннее 100 знаков.
pub fn valid_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && trimmed == name
        && name.chars().count() <= 100
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
}

pub fn account_dir(state: &Shared, account_id: &str) -> PathBuf {
    state.config.users_dir().join(account_id)
}

/// Папка существующего vault аккаунта.
pub fn vault_dir(state: &Shared, account_id: &str, name: &str) -> ApiResult<PathBuf> {
    if !valid_name(name) {
        return Err(ApiError::NoVault);
    }
    let dir = account_dir(state, account_id).join(name);
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) if meta.is_dir() => Ok(dir),
        _ => Err(ApiError::NoVault),
    }
}

/// Открывает vault аккаунта; удалённые в нём заметки — в
/// `<данные>/trash/notes/<аккаунт>/<vault>/`, рядом с корзиной самих vault.
pub async fn open_vault(state: &Shared, account_id: &str, name: &str) -> ApiResult<Arc<OpenVault>> {
    let dir = vault_dir(state, account_id, name)?;
    let trash = state
        .config
        .trash_dir()
        .join("notes")
        .join(account_id)
        .join(name);
    let home = state.sync.home(state, account_id).await?;
    state.vaults.get(account_id, name, &dir, trash, home).await
}

// ── Открытые vault ──────────────────────────────────────────────────────────

/// Открытый vault: состояния команд и рассылка его событий вкладкам (research.md §5).
pub struct OpenVault {
    pub host: Host,
    pub events: broadcast::Sender<Arc<serde_json::Value>>,
    last_used: std::sync::Mutex<Instant>,
    /// Пока жив — жива и пересылка событий; закрытие vault её останавливает.
    _alive: watch::Sender<()>,
}

impl OpenVault {
    pub fn touch(&self) {
        *self.last_used.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }

    fn idle_for(&self) -> Duration {
        self.last_used
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .elapsed()
    }
}

type Key = (String, String);

/// `Host` на пару (аккаунт, vault): живёт, пока им пользуются, и закрывается по
/// простою без подписчиков событий. Замок карты — только на поиск ячейки:
/// открытие большого vault (обход папки) ждут лишь обращения к нему же.
#[derive(Default)]
pub struct OpenVaults(Mutex<HashMap<Key, Arc<OnceCell<Arc<OpenVault>>>>>);

/// Сколько держать vault открытым без обращений и подписчиков.
const IDLE_CLOSE: Duration = Duration::from_secs(10 * 60);
/// Открытых vault на весь сервер: у каждого свой наблюдатель за папкой, а предел
/// наблюдателей ОС общий для процесса (принцип VII — не деградировать молча).
const MAX_OPEN: usize = 256;

impl OpenVaults {
    /// Открытый vault; `trash` — куда класть удалённые заметки, `sync` —
    /// окружение синхронизации аккаунта (фича 038).
    pub async fn get(
        &self,
        account_id: &str,
        name: &str,
        dir: &Path,
        trash: PathBuf,
        sync: Arc<typsidian_host::SyncHome>,
    ) -> ApiResult<Arc<OpenVault>> {
        let key = (account_id.to_owned(), name.to_owned());
        let cell = {
            let mut map = self.0.lock().await;
            if !map.contains_key(&key) && map.len() >= MAX_OPEN {
                map.retain(|_, cell| idle_cell(cell).is_none());
                if map.len() >= MAX_OPEN {
                    return Err(anyhow::anyhow!("открыто vault: {} — предел", map.len()).into());
                }
            }
            map.entry(key).or_default().clone()
        };
        let open = cell
            .get_or_try_init(|| async {
                let mut backend = NativeBackend::open(dir)
                    .await
                    .map_err(|error| anyhow::anyhow!("открытие vault: {error:?}"))?;
                backend.set_trash_dir(trash);
                let ready = backend.events_ready();
                let host = Host {
                    sync_home: sync,
                    ..Host::default()
                };
                *host.vault.0.lock().await = Some(backend);
                let (events, _) = broadcast::channel(256);
                let (alive, closed) = watch::channel(());
                let open = Arc::new(OpenVault {
                    host,
                    events,
                    last_used: std::sync::Mutex::new(Instant::now()),
                    _alive: alive,
                });
                spawn_event_pump(Arc::downgrade(&open), closed, ready);
                anyhow::Ok(open)
            })
            .await?;
        open.touch();
        Ok(open.clone())
    }

    /// Закрывает vault (перед переименованием и удалением папки).
    pub async fn close(&self, account_id: &str, name: &str) {
        self.take(account_id, name).await;
    }

    /// Закрывает vault и отдаёт его, если он был открыт: запросы, которые уже
    /// держат его, ещё могут идти.
    async fn take(&self, account_id: &str, name: &str) -> Option<Arc<OpenVault>> {
        let key = (account_id.to_owned(), name.to_owned());
        let cell = self.0.lock().await.remove(&key)?;
        cell.get().cloned()
    }

    /// Закрывает vault и его сессию синхронизации. Сессию забирают под её
    /// замком: команда, которая уже идёт (подключение удалённого с записью
    /// токена), доработает до конца, а следующая увидит «нет сессии» и ничего
    /// под id удалённого vault не запишет (038, ревью безопасности).
    pub async fn close_with_sync(&self, account_id: &str, name: &str) {
        if let Some(open) = self.take(account_id, name).await {
            // Идущий обмен отменяется, а не дожидается своего предела.
            typsidian_host::commands::sync_cancel(&open.host.sync_cancel);
            let _exchange = open.host.sync_state.1.enter_wait().await;
            *open.host.sync_state.0.lock().await = None;
        }
    }

    /// Закрывает все vault аккаунта.
    pub async fn close_account(&self, account_id: &str) {
        self.0.lock().await.retain(|(id, _), _| id != account_id);
    }

    /// Закрывает простаивающие vault.
    pub async fn sweep(&self) {
        self.0
            .lock()
            .await
            .retain(|_, cell| idle_cell(cell).is_none());
    }
}

/// `Some(())` — ячейку можно убрать: открытие не идёт, vault не открыт или
/// простаивает без подписчиков и запросов.
fn idle_cell(cell: &Arc<OnceCell<Arc<OpenVault>>>) -> Option<()> {
    match cell.get() {
        // Открытие ещё идёт — ячейку держит и тот, кто открывает.
        None => (Arc::strong_count(cell) == 1).then_some(()),
        Some(open) => (open.events.receiver_count() == 0
            && Arc::strong_count(open) == 1
            && open.idle_for() >= IDLE_CLOSE)
            .then_some(()),
    }
}

/// Пересылка событий бэкенда подписчикам — то же, что `spawn_event_forwarding`
/// оболочки Tauri, но вкладкам через SSE.
fn spawn_event_pump(open: Weak<OpenVault>, mut closed: watch::Receiver<()>, ready: Arc<Notify>) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = ready.notified() => {}
                // Отправитель живёт в `OpenVault`: ошибка — vault закрыт.
                _ = closed.changed() => return,
            }
            let Some(open) = open.upgrade() else { return };
            let events = {
                let mut guard = open.host.vault.0.lock().await;
                match guard.as_mut() {
                    Some(backend) => backend.poll_events(),
                    None => return,
                }
            };
            if events.is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::to_value(&events) {
                let _ = open
                    .events
                    .send(Arc::new(serde_json::json!({ "events": value })));
            }
        }
    });
}

// ── Обработчики `/api/vaults` ───────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultView {
    name: String,
    /// Число заметок и время последней правки — для списка (макет S3).
    notes: u32,
    modified: Option<i64>,
}

/// Сколько записей обходить ради подписи в списке: огромный vault не держит ответ.
const SCAN_LIMIT: usize = 50_000;

/// Подпись vault в списке: заметки `.typ` и самая поздняя правка. Служебные папки
/// (с точкой в начале) не считаются.
fn view(name: String, dir: &Path) -> VaultView {
    let mut notes = 0u32;
    let mut modified = std::fs::metadata(dir).ok().as_ref().and_then(modified_secs);
    let mut stack = vec![dir.to_path_buf()];
    let mut seen = 0usize;
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen > SCAN_LIMIT {
                return VaultView {
                    name,
                    notes,
                    modified,
                };
            }
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if entry.path().extension().is_some_and(|ext| ext == "typ") {
                notes += 1;
                modified = modified.max(modified_secs(&meta));
            }
        }
    }
    VaultView {
        name,
        notes,
        modified,
    }
}

fn modified_secs(meta: &std::fs::Metadata) -> Option<i64> {
    meta.modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

pub async fn list(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
) -> ApiResult<Json<Vec<VaultView>>> {
    let dir = account_dir(&state, &authed.account.id);
    // Обход папок — синхронный ввод-вывод: не в рабочих потоках асинхронной среды.
    let out = tokio::task::spawn_blocking(move || list_dir(&dir))
        .await
        .map_err(|e| anyhow::anyhow!(e))??;
    Ok(Json(out))
}

fn list_dir(dir: &Path) -> std::io::Result<Vec<VaultView>> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(error) => return Err(error),
    };
    for entry in entries.flatten().take(MAX_VAULTS) {
        let Ok(meta) = entry.metadata() else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if meta.is_dir() && valid_name(&name) {
            out.push(view(name, &entry.path()));
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Vault у одного аккаунта не больше: список обходит каждый.
const MAX_VAULTS: usize = 100;

#[derive(Deserialize)]
pub struct NameRequest {
    name: String,
}

fn check_free(state: &Shared, account_id: &str, name: &str) -> ApiResult<PathBuf> {
    if !valid_name(name) {
        return Err(ApiError::Invalid {
            field: "name",
            reason: "BadName",
        });
    }
    let dir = account_dir(state, account_id).join(name);
    if std::fs::symlink_metadata(&dir).is_ok() {
        return Err(ApiError::Invalid {
            field: "name",
            reason: "Taken",
        });
    }
    Ok(dir)
}

pub async fn create(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
    Json(request): Json<NameRequest>,
) -> ApiResult<Json<VaultView>> {
    let dir = check_free(&state, &authed.account.id, &request.name)?;
    let parent = account_dir(&state, &authed.account.id);
    let count = std::fs::read_dir(&parent).map(|e| e.count()).unwrap_or(0);
    if count >= MAX_VAULTS {
        return Err(ApiError::Invalid {
            field: "name",
            reason: "TooMany",
        });
    }
    crate::private_dir(&parent)?;
    // `create_dir`, а не `create_dir_all`: уже существующая папка — отказ, а не
    // чужой vault под тем же именем (гонка с параллельным созданием).
    std::fs::create_dir(&dir).map_err(|error| match error.kind() {
        std::io::ErrorKind::AlreadyExists => ApiError::Invalid {
            field: "name",
            reason: "Taken",
        },
        _ => error.into(),
    })?;
    // Строка id прежнего vault с этим именем (удалённого мимо сервера) не должна
    // отдать новому его токен и зеркало (038, ревью безопасности).
    forget_sync(&state, &authed.account.id, &request.name).await?;
    tracing::info!(login = %authed.account.login, "vault создан");
    Ok(Json(view(request.name, &dir)))
}

pub async fn rename(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
    UrlPath(name): UrlPath<String>,
    Json(request): Json<NameRequest>,
) -> ApiResult<Json<VaultView>> {
    let from = vault_dir(&state, &authed.account.id, &name)?;
    let to = check_free(&state, &authed.account.id, &request.name)?;
    // Место занимается атомарно пустой папкой, и `rename` заменяет только её: vault,
    // созданный под тем же именем между проверкой и переносом, не затрётся.
    std::fs::create_dir(&to).map_err(|error| match error.kind() {
        std::io::ErrorKind::AlreadyExists => ApiError::Invalid {
            field: "name",
            reason: "Taken",
        },
        _ => error.into(),
    })?;
    state.vaults.close(&authed.account.id, &name).await;
    // Id синхронизации идёт за vault: его секреты и зеркало остаются при нём.
    // Переносится до папки — иначе сбой базы оторвал бы id от уже
    // переименованного vault.
    let moved = async {
        forget_sync(&state, &authed.account.id, &request.name).await?;
        state
            .store
            .rename_vault_sync_id(&authed.account.id, &name, &request.name)
            .await
    }
    .await;
    if let Err(error) = moved {
        let _ = std::fs::remove_dir(&to);
        return Err(error.into());
    }
    if let Err(error) = std::fs::rename(&from, &to) {
        let _ = std::fs::remove_dir(&to);
        let _ = state
            .store
            .rename_vault_sync_id(&authed.account.id, &request.name, &name)
            .await;
        return Err(error.into());
    }
    tracing::info!(login = %authed.account.login, "vault переименован");
    Ok(Json(view(request.name, &to)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSummary {
    name: String,
    notes: u32,
}

/// Число заметок — для подтверждения удаления (clarifications).
pub async fn summary(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
    UrlPath(name): UrlPath<String>,
) -> ApiResult<Json<VaultSummary>> {
    let open = open_vault(&state, &authed.account.id, &name).await?;
    let notes = {
        let guard = open.host.vault.0.lock().await;
        match guard.as_ref() {
            Some(backend) => backend
                .list()
                .await
                .iter()
                .filter(|note| note.kind == typsidian_core::vault::FileKind::Note)
                .count() as u32,
            None => 0,
        }
    };
    Ok(Json(VaultSummary { name, notes }))
}

/// Удаление — в корзину сервера, откуда администратор может вернуть папку.
pub async fn delete(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
    UrlPath(name): UrlPath<String>,
) -> ApiResult<Json<()>> {
    let from = vault_dir(&state, &authed.account.id, &name)?;
    state
        .vaults
        .close_with_sync(&authed.account.id, &name)
        .await;
    let trash = state.config.trash_dir();
    crate::private_dir(&trash)?;
    // Случайный хвост: два удаления за секунду не встретятся в одном имени.
    let target = trash.join(format!(
        "{}-{}-{}-{}",
        crate::store::now(),
        authed.account.id,
        name,
        &crate::store::random_token()[..8]
    ));
    std::fs::rename(&from, &target)?;
    // Секреты синхронизации vault в корзину не идут (038 FR-002): удаляются
    // сразу вместе с зеркалом и id.
    forget_sync(&state, &authed.account.id, &name).await?;
    tracing::info!(login = %authed.account.login, "vault удалён в корзину");
    Ok(Json(()))
}

/// Забывает id синхронизации vault с этим именем, его секреты и зеркало.
async fn forget_sync(state: &Shared, account_id: &str, name: &str) -> anyhow::Result<()> {
    if let Some(sync_id) = state.store.forget_vault_sync_id(account_id, name).await? {
        state.sync.forget_vault(state, account_id, &sync_id).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::valid_name;

    #[test]
    fn vault_names() {
        assert!(valid_name("Диссертация"));
        assert!(valid_name("Статьи 2026"));
        for bad in [
            "", " x", "x ", ".", "..", ".git", "a/b", "a\\b", "x\u{0}", "a:b", "x.",
        ] {
            assert!(!valid_name(bad), "{bad:?}");
        }
        assert!(!valid_name(&"я".repeat(101)));
    }
}
