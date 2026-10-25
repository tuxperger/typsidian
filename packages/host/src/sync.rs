//! Окружение и состояния синхронизации (фича 011; общие для оболочки и сервера с
//! фичи 038, research.md §1–§2).
//!
//! Команды синхронизации — тонкие обёртки над `typsidian_core::sync::SyncSession`
//! (`commands.rs`). Здесь — то, без чего их не вызвать: где лежат настройки
//! устройства, секреты и зеркала ([`SyncHome`]), открытая сессия и помощники,
//! которые собирают контекст вызова и сохраняют изменённые сессией значения
//! устройства.
//!
//! Порядок блокировок один на все команды — устройство, затем vault, затем
//! сессия, — и устройство отпускается до обращения к vault: иначе две команды,
//! взявшие их в разном порядке, ждали бы друг друга вечно.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::Mutex;

use typsidian_core::settings::native::NativeStore;
use typsidian_core::settings::store::{SettingsStore, StoreError};
use typsidian_core::settings::VaultId;
use typsidian_core::sync::engine::native::NativeGitEngine;
use typsidian_core::sync::engine::CancelToken;
use typsidian_core::sync::secrets::{
    entry_name, MemorySecretStore, SecretKind, SecretStore, SecretStoreError,
};
use typsidian_core::sync::{
    self, DeviceVaultSync, ShellCapabilities, SyncContext, SyncError, SyncSession,
};
use typsidian_core::vault::backend::VaultBackend;

use crate::Vault;

// ── Хранилище секретов ──────────────────────────────────────────────────────

/// Будущее операции с секретом. `Send`: его ждут и команды Tauri, и обработчики
/// сервера.
pub type SecretFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, SecretStoreError>> + Send + 'a>>;

/// [`SecretStore`] ядра, который можно держать за `dyn`: у оболочки секреты в
/// keyring ОС, у сервера — в файле аккаунта, а состояние одно.
pub trait DynSecretStore: Send + Sync {
    fn get<'a>(&'a self, vault_id: &'a str, kind: SecretKind) -> SecretFuture<'a, Option<String>>;
    fn set<'a>(
        &'a self,
        vault_id: &'a str,
        kind: SecretKind,
        value: &'a str,
    ) -> SecretFuture<'a, ()>;
    fn delete<'a>(&'a self, vault_id: &'a str, kind: SecretKind) -> SecretFuture<'a, ()>;
}

/// Хранилище секретов синхронизации, которое передаётся сессии.
#[derive(Clone)]
pub struct DynSecrets(pub Arc<dyn DynSecretStore>);

impl SecretStore for DynSecrets {
    async fn get(
        &self,
        vault_id: &str,
        kind: SecretKind,
    ) -> Result<Option<String>, SecretStoreError> {
        self.0.get(vault_id, kind).await
    }

    async fn set(
        &self,
        vault_id: &str,
        kind: SecretKind,
        value: &str,
    ) -> Result<(), SecretStoreError> {
        self.0.set(vault_id, kind, value).await
    }

    async fn delete(&self, vault_id: &str, kind: SecretKind) -> Result<(), SecretStoreError> {
        self.0.delete(vault_id, kind).await
    }
}

/// Секреты в памяти до закрытия процесса — там, где хранилища нет.
struct Memory(MemorySecretStore);

impl DynSecretStore for Memory {
    fn get<'a>(&'a self, vault_id: &'a str, kind: SecretKind) -> SecretFuture<'a, Option<String>> {
        Box::pin(async move { self.0.get(vault_id, kind).await })
    }

    fn set<'a>(
        &'a self,
        vault_id: &'a str,
        kind: SecretKind,
        value: &'a str,
    ) -> SecretFuture<'a, ()> {
        Box::pin(async move { self.0.set(vault_id, kind, value).await })
    }

    fn delete<'a>(&'a self, vault_id: &'a str, kind: SecretKind) -> SecretFuture<'a, ()> {
        Box::pin(async move { self.0.delete(vault_id, kind).await })
    }
}

/// Все виды секретов: удаление vault удаляет каждый.
const ALL_KINDS: [SecretKind; 4] = [
    SecretKind::RemoteUser,
    SecretKind::RemoteToken,
    SecretKind::RemoteOrigin,
    SecretKind::SyncthingApiKey,
];

/// Секреты в файле JSON с правами только для владельца: приватный файл
/// приложения на Android, файл аккаунта на сервере (038 research.md §2).
pub struct FileSecrets {
    path: PathBuf,
    lock: Mutex<()>,
}

type Entries = std::collections::BTreeMap<String, String>;

impl FileSecrets {
    pub fn new(path: PathBuf) -> Self {
        FileSecrets {
            path,
            lock: Mutex::new(()),
        }
    }

    /// Удаляет все секреты vault (удаление vault на сервере).
    pub async fn forget_vault(&self, vault_id: &str) -> Result<(), SecretStoreError> {
        let _guard = self.lock.lock().await;
        let mut entries = entries_for_change(&self.path)?;
        let before = entries.len();
        for kind in ALL_KINDS {
            entries.remove(&entry_name(vault_id, kind));
        }
        if entries.len() != before {
            write_file_secrets(&self.path, &entries)?;
        }
        Ok(())
    }
}

/// Файл секретов: нет файла — пусто; файл есть, но не читается или не
/// разбирается — отказ, а не «пусто» (011 T077): иначе следующая запись затёрла
/// бы его вместе со всеми секретами.
fn read_file_secrets(path: &std::path::Path) -> Result<Entries, SecretStoreError> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|_| SecretStoreError),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
        Err(_) => Err(SecretStoreError),
    }
}

/// Файл есть, прочитан, но не разбирается — повреждён. Ошибка чтения (нет
/// дескрипторов, нет прав, сбой диска) повреждением не считается: иначе секреты
/// всех vault пропали бы из-за мгновенной нехватки дескрипторов (038).
fn is_corrupt(path: &std::path::Path) -> bool {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<Entries>(&bytes).is_err(),
        Err(_) => false,
    }
}

/// Записи для изменения. Повреждённый файл откладывается рядом
/// (`….corrupt-<время>.json`), а не перезаписывается: секреты в нём, возможно,
/// ещё можно достать, а новые пишутся в чистый файл.
fn entries_for_change(path: &std::path::Path) -> Result<Entries, SecretStoreError> {
    match read_file_secrets(path) {
        Ok(entries) => Ok(entries),
        Err(_) if is_corrupt(path) => {
            std::fs::rename(path, path.with_extension(format!("corrupt-{}.json", now())))
                .map_err(|_| SecretStoreError)?;
            Ok(Default::default())
        }
        Err(error) => Err(error),
    }
}

fn write_file_secrets(path: &std::path::Path, entries: &Entries) -> Result<(), SecretStoreError> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(parent).map_err(|_| SecretStoreError)?;
    }
    let text = serde_json::to_string(entries).map_err(|_| SecretStoreError)?;
    let temporary = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|_| SecretStoreError)?;
    file.write_all(text.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|_| SecretStoreError)?;
    std::fs::rename(&temporary, path).map_err(|_| SecretStoreError)
}

impl DynSecretStore for FileSecrets {
    fn get<'a>(&'a self, vault_id: &'a str, kind: SecretKind) -> SecretFuture<'a, Option<String>> {
        Box::pin(async move {
            let _guard = self.lock.lock().await;
            Ok(read_file_secrets(&self.path)?
                .get(&entry_name(vault_id, kind))
                .cloned())
        })
    }

    fn set<'a>(
        &'a self,
        vault_id: &'a str,
        kind: SecretKind,
        value: &'a str,
    ) -> SecretFuture<'a, ()> {
        Box::pin(async move {
            // Прочитать-изменить-записать под одной блокировкой: две записи
            // подряд иначе теряли бы одна другую.
            let _guard = self.lock.lock().await;
            let mut entries = entries_for_change(&self.path)?;
            entries.insert(entry_name(vault_id, kind), value.to_string());
            write_file_secrets(&self.path, &entries)
        })
    }

    fn delete<'a>(&'a self, vault_id: &'a str, kind: SecretKind) -> SecretFuture<'a, ()> {
        Box::pin(async move {
            let _guard = self.lock.lock().await;
            let mut entries = entries_for_change(&self.path)?;
            if entries.remove(&entry_name(vault_id, kind)).is_some() {
                write_file_secrets(&self.path, &entries)?;
            }
            Ok(())
        })
    }
}

// ── Окружение ───────────────────────────────────────────────────────────────

/// Где открыть настройки устройства.
pub enum DevicePlace {
    /// Синхронизация здесь недоступна (`SyncHome::default()`).
    Nowhere,
    /// Папка настроек ОС, а если её нет (Android) — запасная папка приложения.
    System { fallback: Option<PathBuf> },
    /// Своя папка: на сервере — папка аккаунта.
    In(PathBuf),
}

/// Окружение синхронизации: настройки устройства, секреты, зеркала и
/// возможности. Оболочка собирает его из папки приложения и keyring, сервер — из
/// папки данных аккаунта (один на аккаунт: его настройки устройства общие для
/// открытых vault).
pub struct SyncHome {
    /// Открытое хранилище настроек устройства. Тот же `Mutex`, что у состояния
    /// `Device` оболочки: две копии одного файла теряли бы записи друг друга.
    pub device: Arc<Mutex<Option<NativeStore>>>,
    place: DevicePlace,
    mirrors: Option<PathBuf>,
    secrets: DynSecrets,
    capabilities: ShellCapabilities,
}

impl Default for SyncHome {
    fn default() -> Self {
        SyncHome {
            device: Arc::default(),
            place: DevicePlace::Nowhere,
            mirrors: None,
            secrets: DynSecrets(Arc::new(Memory(MemorySecretStore::new()))),
            capabilities: ShellCapabilities::current(cfg!(target_os = "android")),
        }
    }
}

impl SyncHome {
    pub fn new(
        device: Arc<Mutex<Option<NativeStore>>>,
        place: DevicePlace,
        mirrors: Option<PathBuf>,
        secrets: Arc<dyn DynSecretStore>,
        capabilities: ShellCapabilities,
    ) -> Self {
        SyncHome {
            device,
            place,
            mirrors,
            secrets: DynSecrets(secrets),
            capabilities,
        }
    }

    pub fn capabilities(&self) -> ShellCapabilities {
        self.capabilities
    }

    pub fn secrets(&self) -> &DynSecrets {
        &self.secrets
    }

    /// Хранилище устройства; открывает его при первом вопросе.
    pub async fn device_store<'a>(
        &self,
        guard: &'a mut Option<NativeStore>,
    ) -> Result<&'a mut NativeStore, StoreError> {
        if guard.is_none() {
            *guard = Some(match &self.place {
                DevicePlace::Nowhere => return Err(StoreError::NoConfigDirectory),
                DevicePlace::System { fallback } => match NativeStore::open().await {
                    Err(StoreError::NoConfigDirectory) => {
                        let dir = fallback.clone().ok_or(StoreError::NoConfigDirectory)?;
                        NativeStore::open_in(dir).await?
                    }
                    other => other?,
                },
                DevicePlace::In(dir) => NativeStore::open_in(dir.clone()).await?,
            });
        }
        Ok(guard.as_mut().expect("хранилище только что открыто"))
    }
}

// ── Сессия ──────────────────────────────────────────────────────────────────

/// Сессия синхронизации открытого vault.
#[derive(Default)]
pub struct SyncState(pub Mutex<Option<OpenSync>>, pub ExchangeGate);

/// Одна сетевая операция на сессию (038, ревью безопасности): сетевой шаг идёт без
/// блокировки сессии, и без этого параллельные обмены одного vault занимали бы
/// поток каждый и сбрасывали бы отмену друг друга.
#[derive(Default)]
pub struct ExchangeGate(Mutex<()>);

impl ExchangeGate {
    /// Вход в обмен; идёт другой — отказ `ExchangeRunning`.
    pub fn enter(&self) -> SyncAnswer<tokio::sync::MutexGuard<'_, ()>> {
        self.0
            .try_lock()
            .map_err(|_| SyncFailure::Core(SyncError::ExchangeRunning))
    }

    /// Дождаться конца идущего обмена (закрытие vault).
    pub async fn enter_wait(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.0.lock().await
    }
}

pub struct OpenSync {
    pub session: SyncSession<NativeGitEngine>,
    pub vault: VaultId,
}

/// Токен отмены текущей операции — отдельно от сессии: пока операция идёт,
/// сессия заблокирована, а отменить её нужно именно тогда.
#[derive(Default)]
pub struct SyncCancel(pub std::sync::Mutex<Option<CancelToken>>);

/// Ход массовой операции открытой сессии (011 T075) — отдельно от сессии по той
/// же причине, что токен отмены: читается, пока операция идёт.
#[derive(Default)]
pub struct SyncProgressSlot(pub std::sync::Mutex<Option<sync::ProgressSink>>);

/// Отказ команды синхронизации.
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum SyncFailure {
    NotOpen(NotOpen),
    Core(SyncError),
}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind")]
pub enum NotOpen {
    NoVaultOpen,
}

impl From<SyncError> for SyncFailure {
    fn from(error: SyncError) -> Self {
        SyncFailure::Core(error)
    }
}

pub type SyncAnswer<T> = Result<T, SyncFailure>;

pub fn not_open() -> SyncFailure {
    SyncFailure::NotOpen(NotOpen::NoVaultOpen)
}

/// Идентификатор vault идёт в имя каталога зеркала и в имя записи секрета —
/// поэтому только в том виде, в каком его выдаёт реестр ядра
/// (`VaultId::mint`): шестнадцатеричная строка без разделителей (011 T068).
pub fn require_vault_id(vault_id: &str) -> Result<(), SyncFailure> {
    let well_formed =
        (1..=64).contains(&vault_id.len()) && vault_id.bytes().all(|b| b.is_ascii_hexdigit());
    if well_formed {
        Ok(())
    } else {
        Err(SyncError::Io {
            message: "неизвестный vault".to_string(),
        }
        .into())
    }
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

fn device_unavailable(_: StoreError) -> SyncFailure {
    SyncError::Io {
        message: "настройки устройства недоступны".to_string(),
    }
    .into()
}

/// Открывает сессию синхронизации для открытого vault. Оболочка зовёт это из
/// своей команды `sync_open` (и запускает наблюдатель Syncthing), сервер — с id
/// vault, который назначил сам.
pub async fn open_session(
    vault: &Vault,
    home: &SyncHome,
    sync_state: &SyncState,
    cancel: &SyncCancel,
    progress: &SyncProgressSlot,
    vault_id: String,
) -> SyncAnswer<()> {
    require_vault_id(&vault_id)?;
    let root = {
        let guard = vault.0.lock().await;
        guard
            .as_ref()
            .ok_or_else(not_open)?
            .root_path()
            .to_path_buf()
    };
    let mirror_dir = home
        .mirrors
        .as_ref()
        .map(|dir| dir.join(format!("{vault_id}.git")));
    let session = SyncSession::new(
        NativeGitEngine::open(root),
        mirror_dir.map(NativeGitEngine::mirror),
        vault_id.clone(),
    );
    if let Ok(mut slot) = cancel.0.lock() {
        *slot = Some(session.cancel_token());
    }
    if let Ok(mut slot) = progress.0.lock() {
        *slot = Some(session.progress_sink());
    }
    *sync_state.0.lock().await = Some(OpenSync {
        session,
        vault: VaultId::from_raw(vault_id),
    });
    Ok(())
}

/// Папка зеркала vault — чтобы удалить её вместе с vault.
pub fn mirror_dir(mirrors: &std::path::Path, vault_id: &str) -> Option<PathBuf> {
    require_vault_id(vault_id).ok()?;
    Some(mirrors.join(format!("{vault_id}.git")))
}

/// Контекст вызова из настроек устройства.
pub async fn context(home: &SyncHome, vault: &VaultId) -> Result<SyncContext, SyncFailure> {
    let mut guard = home.device.lock().await;
    let store = home
        .device_store(&mut guard)
        .await
        .map_err(device_unavailable)?;
    // Тождество устройства выдаётся при первом обращении (011 research §4).
    if let Some(change) = sync::ensure_device_change(SettingsStore::settings(store)) {
        store.apply(&change).await;
    }
    Ok(sync::context_for(
        SettingsStore::settings(store),
        vault,
        now(),
        home.capabilities,
    ))
}

/// Id vault открытой сессии.
pub async fn open_vault_id(sync_state: &SyncState) -> SyncAnswer<VaultId> {
    let guard = sync_state.0.lock().await;
    Ok(guard.as_ref().ok_or_else(not_open)?.vault.clone())
}

/// Контекст команды по идентификатору открытого vault.
pub async fn sync_context(home: &SyncHome, sync_state: &SyncState) -> SyncAnswer<SyncContext> {
    let vault_id = open_vault_id(sync_state).await?;
    context(home, &vault_id).await
}

/// Сохраняет значения устройства, изменённые сессией.
pub async fn persist_device_update(home: &SyncHome, sync_state: &SyncState) {
    let (update, vault) = {
        let mut guard = sync_state.0.lock().await;
        let Some(open) = guard.as_mut() else {
            return;
        };
        (open.session.take_device_update(), open.vault.clone())
    };
    let Some(update) = update else {
        return;
    };
    store_device_values(home, vault, update).await;
}

async fn store_device_values(home: &SyncHome, vault: VaultId, value: DeviceVaultSync) {
    let mut guard = home.device.lock().await;
    if let Ok(store) = home.device_store(&mut guard).await {
        store.apply(&sync::device_sync_change(vault, value)).await;
    }
}

/// Значения устройства открытого vault (автор снимков и прочее).
pub async fn device_values(home: &SyncHome, sync_state: &SyncState) -> SyncAnswer<DeviceVaultSync> {
    Ok(sync_context(home, sync_state).await?.device_sync)
}

/// Меняет значения устройства открытого vault.
pub async fn change_device_values(
    home: &SyncHome,
    sync_state: &SyncState,
    change: impl FnOnce(&mut DeviceVaultSync),
) -> SyncAnswer<()> {
    let vault = open_vault_id(sync_state).await?;
    let mut value = context(home, &vault).await?.device_sync;
    change(&mut value);
    store_device_values(home, vault, value.normalized()).await;
    Ok(())
}

/// Тело команды над открытыми vault и сессией: контекст, вызов, сохранение
/// значений устройства.
#[macro_export]
macro_rules! with_sync {
    ($home:expr, $vault:expr, $sync:expr, |$backend:ident, $session:ident, $ctx:ident| $body:expr) => {{
        let vault_id = $crate::sync::open_vault_id($sync).await?;
        #[allow(unused_variables)]
        let $ctx = $crate::sync::context($home, &vault_id).await?;
        let result = {
            let mut vault_guard = $vault.0.lock().await;
            let $backend = vault_guard.as_mut().ok_or_else($crate::sync::not_open)?;
            let mut sync_guard = $sync.0.lock().await;
            let open = sync_guard.as_mut().ok_or_else($crate::sync::not_open)?;
            let $session = &mut open.session;
            $body
        };
        $crate::sync::persist_device_update($home, $sync).await;
        result
    }};
}

/// Шаг под обеими блокировками — тот же порядок, что у `with_sync!`: vault,
/// затем сессия. Сетевой шаг обмена идёт между такими шагами без блокировок
/// (011 T071): сохранение заметок и опрос состояния не ждут сети.
#[macro_export]
macro_rules! sync_locked {
    ($vault:expr, $sync:expr, |$backend:ident, $session:ident| $body:expr) => {{
        let mut vault_guard = $vault.0.lock().await;
        #[allow(unused_variables)]
        let $backend = vault_guard.as_mut().ok_or_else($crate::sync::not_open)?;
        let mut sync_guard = $sync.0.lock().await;
        let open = sync_guard.as_mut().ok_or_else($crate::sync::not_open)?;
        let $session = &mut open.session;
        $body
    }};
}

/// Чтение под обеими блокировками без изменения.
#[macro_export]
macro_rules! sync_read {
    ($vault:expr, $sync:expr, |$backend:ident, $session:ident| $body:expr) => {{
        let vault_guard = $vault.0.lock().await;
        let $backend = vault_guard.as_ref().ok_or_else($crate::sync::not_open)?;
        let sync_guard = $sync.0.lock().await;
        let open = sync_guard.as_ref().ok_or_else($crate::sync::not_open)?;
        let $session = &open.session;
        $body
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "typsidian-host-{name}-{}-{}",
            std::process::id(),
            now()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn file_secrets_are_private_and_per_vault() {
        let dir = temp_dir("secrets");
        let path = dir.join("secrets.json");
        let store = FileSecrets::new(path.clone());
        store
            .set("aa", SecretKind::RemoteToken, "token-a")
            .await
            .unwrap();
        store
            .set("bb", SecretKind::RemoteToken, "token-b")
            .await
            .unwrap();
        store
            .set("aa", SecretKind::RemoteOrigin, "https://example.com/a.git")
            .await
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        store.forget_vault("aa").await.unwrap();
        assert_eq!(
            store.get("aa", SecretKind::RemoteToken).await.unwrap(),
            None
        );
        assert_eq!(
            store.get("aa", SecretKind::RemoteOrigin).await.unwrap(),
            None
        );
        assert_eq!(
            store
                .get("bb", SecretKind::RemoteToken)
                .await
                .unwrap()
                .as_deref(),
            Some("token-b")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn a_corrupt_secrets_file_is_set_aside_not_overwritten() {
        let dir = temp_dir("corrupt");
        let path = dir.join("secrets.json");
        std::fs::write(&path, "{не json").unwrap();
        let store = FileSecrets::new(path.clone());
        assert!(store.get("aa", SecretKind::RemoteToken).await.is_err());
        store.set("aa", SecretKind::RemoteToken, "t").await.unwrap();
        let aside: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("secrets.corrupt-")
            })
            .collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(
            std::fs::read_to_string(aside[0].path()).unwrap(),
            "{не json"
        );
        assert_eq!(
            store
                .get("aa", SecretKind::RemoteToken)
                .await
                .unwrap()
                .as_deref(),
            Some("t")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn without_a_place_sync_answers_instead_of_panicking() {
        let home = SyncHome::default();
        let state = SyncState::default();
        assert!(sync_context(&home, &state).await.is_err());
        *state.0.lock().await = None;
        assert!(context(&home, &VaultId::from_raw("ab".to_string()))
            .await
            .is_err());
    }
}
