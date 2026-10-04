//! Шов к защищённому хранилищу устройства (фича 011, research §10, FR-014).
//!
//! Токены и ключи **никогда** не попадают в файлы vault, в адрес удалённого
//! репозитория, в сообщения об ошибках и в журналы. Хранилище — свойство
//! устройства, поэтому реализации живут там, где известна платформа:
//! keyring ОС на рабочем столе, приватный каталог приложения на Android
//! (`packages/shell`), IndexedDB браузера в web ([`web`]).

use serde::{Deserialize, Serialize};

/// Что за секрет.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SecretKind {
    RemoteUser,
    RemoteToken,
    SyncthingApiKey,
}

impl SecretKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SecretKind::RemoteUser => "remote-user",
            SecretKind::RemoteToken => "remote-token",
            SecretKind::SyncthingApiKey => "syncthing-api-key",
        }
    }
}

/// Имя записи в хранилище: vault и вид секрета.
pub fn entry_name(vault_id: &str, kind: SecretKind) -> String {
    format!("{vault_id}/{}", kind.as_str())
}

/// Отказ хранилища. Без подробностей: их источник может содержать сам секрет.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("хранилище секретов недоступно")]
pub struct SecretStoreError;

#[allow(async_fn_in_trait)]
pub trait SecretStore {
    async fn get(
        &self,
        vault_id: &str,
        kind: SecretKind,
    ) -> Result<Option<String>, SecretStoreError>;
    async fn set(
        &self,
        vault_id: &str,
        kind: SecretKind,
        value: &str,
    ) -> Result<(), SecretStoreError>;
    async fn delete(&self, vault_id: &str, kind: SecretKind) -> Result<(), SecretStoreError>;
}

/// Хранилище в памяти: для тестов и как запасной путь, когда хранилище ОС
/// недоступно, — секрет тогда живёт до закрытия приложения, а не на диске.
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    entries: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    async fn get(
        &self,
        vault_id: &str,
        kind: SecretKind,
    ) -> Result<Option<String>, SecretStoreError> {
        Ok(self
            .entries
            .lock()
            .map_err(|_| SecretStoreError)?
            .get(&entry_name(vault_id, kind))
            .cloned())
    }

    async fn set(
        &self,
        vault_id: &str,
        kind: SecretKind,
        value: &str,
    ) -> Result<(), SecretStoreError> {
        self.entries
            .lock()
            .map_err(|_| SecretStoreError)?
            .insert(entry_name(vault_id, kind), value.to_string());
        Ok(())
    }

    async fn delete(&self, vault_id: &str, kind: SecretKind) -> Result<(), SecretStoreError> {
        self.entries
            .lock()
            .map_err(|_| SecretStoreError)?
            .remove(&entry_name(vault_id, kind));
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
pub mod web {
    //! Секреты в IndexedDB origin-а приложения. Браузер не даёт защищённого
    //! хранилища — интерфейс говорит об этом при вводе токена
    //! (`ShellCapabilities::secure_secrets = false`).

    use super::*;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    const DB_NAME: &str = "typsidian-secrets";
    const STORE: &str = "secrets";

    #[derive(Debug, Default, Clone, Copy)]
    pub struct IdbSecretStore;

    fn request_promise(request: &web_sys::IdbRequest) -> js_sys::Promise {
        let request = request.clone();
        js_sys::Promise::new(&mut |resolve, reject| {
            let ok_request = request.clone();
            let on_success = wasm_bindgen::closure::Closure::once_into_js(move || {
                let _ = resolve.call1(
                    &JsValue::NULL,
                    &ok_request.result().unwrap_or(JsValue::NULL),
                );
            });
            let on_error = wasm_bindgen::closure::Closure::once_into_js(move || {
                let _ = reject.call0(&JsValue::NULL);
            });
            request.set_onsuccess(Some(on_success.unchecked_ref()));
            request.set_onerror(Some(on_error.unchecked_ref()));
        })
    }

    async fn open() -> Result<web_sys::IdbDatabase, SecretStoreError> {
        let factory = web_sys::window()
            .and_then(|window| window.indexed_db().ok().flatten())
            .ok_or(SecretStoreError)?;
        let request = factory
            .open_with_u32(DB_NAME, 1)
            .map_err(|_| SecretStoreError)?;
        let upgrade_request = request.clone();
        let on_upgrade = wasm_bindgen::closure::Closure::once_into_js(move || {
            if let Ok(result) = upgrade_request.result() {
                let db: web_sys::IdbDatabase = result.unchecked_into();
                if !db.object_store_names().contains(STORE) {
                    let _ = db.create_object_store(STORE);
                }
            }
        });
        request.set_onupgradeneeded(Some(on_upgrade.unchecked_ref()));
        let result = JsFuture::from(request_promise(&request))
            .await
            .map_err(|_| SecretStoreError)?;
        Ok(result.unchecked_into())
    }

    async fn with_store<T>(
        mode: web_sys::IdbTransactionMode,
        action: impl FnOnce(&web_sys::IdbObjectStore) -> Result<web_sys::IdbRequest, JsValue>,
        map: impl FnOnce(JsValue) -> T,
    ) -> Result<T, SecretStoreError> {
        let db = open().await?;
        let transaction = db
            .transaction_with_str_and_mode(STORE, mode)
            .map_err(|_| SecretStoreError)?;
        let store = transaction
            .object_store(STORE)
            .map_err(|_| SecretStoreError)?;
        let request = action(&store).map_err(|_| SecretStoreError)?;
        let value = JsFuture::from(request_promise(&request))
            .await
            .map_err(|_| SecretStoreError)?;
        db.close();
        Ok(map(value))
    }

    impl SecretStore for IdbSecretStore {
        async fn get(
            &self,
            vault_id: &str,
            kind: SecretKind,
        ) -> Result<Option<String>, SecretStoreError> {
            let key = JsValue::from_str(&entry_name(vault_id, kind));
            with_store(
                web_sys::IdbTransactionMode::Readonly,
                |store| store.get(&key),
                |value| value.as_string(),
            )
            .await
        }

        async fn set(
            &self,
            vault_id: &str,
            kind: SecretKind,
            value: &str,
        ) -> Result<(), SecretStoreError> {
            let key = JsValue::from_str(&entry_name(vault_id, kind));
            let value = JsValue::from_str(value);
            with_store(
                web_sys::IdbTransactionMode::Readwrite,
                |store| store.put_with_key(&value, &key),
                |_| (),
            )
            .await
        }

        async fn delete(&self, vault_id: &str, kind: SecretKind) -> Result<(), SecretStoreError> {
            let key = JsValue::from_str(&entry_name(vault_id, kind));
            with_store(
                web_sys::IdbTransactionMode::Readwrite,
                |store| store.delete(&key),
                |_| (),
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip() {
        let store = MemorySecretStore::new();
        let run = async {
            store
                .set("v1", SecretKind::RemoteToken, "t0ken")
                .await
                .unwrap();
            assert_eq!(
                store.get("v1", SecretKind::RemoteToken).await.unwrap(),
                Some("t0ken".to_string())
            );
            assert_eq!(
                store.get("v2", SecretKind::RemoteToken).await.unwrap(),
                None
            );
            store.delete("v1", SecretKind::RemoteToken).await.unwrap();
            assert_eq!(
                store.get("v1", SecretKind::RemoteToken).await.unwrap(),
                None
            );
        };
        futures_lite_block_on(run);
    }

    /// Минимальный исполнитель для тестов без рантайма: будущие хранилища в
    /// памяти готовы с первого опроса.
    fn futures_lite_block_on<F: std::future::Future>(future: F) -> F::Output {
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
        fn noop(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
                return value;
            }
        }
    }
}
