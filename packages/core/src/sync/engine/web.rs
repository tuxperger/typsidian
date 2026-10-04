//! Движок git в браузере — isomorphic-git за JS-адаптером (research §3).
//!
//! Адаптер (`ui/src/sync/git-engine-web.ts`) — объект с методами, имена и
//! аргументы которых повторяют [`GitEngine`]; значения пересекают границу в
//! форме, заданной типами Rust (`tsify`, принцип IV). Решений адаптер не
//! принимает: он переводит вызовы в isomorphic-git поверх папки пользователя.
//!
//! Ошибку адаптер бросает объектом формы [`EngineError`] (`{kind: "Offline"}`);
//! всё прочее становится `Io` с текстом исключения.

use js_sys::{Array, Function, Promise, Reflect};
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use super::*;
use crate::sync::Author;

/// JS-объект адаптера.
#[derive(Clone)]
pub struct JsGitEngine {
    adapter: JsValue,
}

impl JsGitEngine {
    pub fn new(adapter: JsValue) -> Self {
        Self { adapter }
    }

    async fn call(&self, method: &str, args: &[JsValue]) -> Result<JsValue, EngineError> {
        let function: Function = Reflect::get(&self.adapter, &JsValue::from_str(method))
            .ok()
            .and_then(|value| value.dyn_into().ok())
            .ok_or_else(|| EngineError::Io {
                message: format!("адаптер git без метода {method}"),
            })?;
        let array: Array = args.iter().collect();
        let result = function
            .apply(&self.adapter, &array)
            .map_err(engine_error)?;
        if result.has_type::<Promise>() {
            JsFuture::from(Promise::from(result))
                .await
                .map_err(engine_error)
        } else {
            Ok(result)
        }
    }

    async fn call_as<T: DeserializeOwned>(
        &self,
        method: &str,
        args: &[JsValue],
    ) -> Result<T, EngineError> {
        let value = self.call(method, args).await?;
        serde_wasm_bindgen::from_value(value).map_err(|error| EngineError::Io {
            message: format!("{method}: {error}"),
        })
    }

    /// Просит адаптер прервать текущую сетевую операцию.
    pub fn abort(&self) {
        if let Ok(function) = Reflect::get(&self.adapter, &JsValue::from_str("abort")) {
            if let Ok(function) = function.dyn_into::<Function>() {
                let _ = function.call0(&self.adapter);
            }
        }
    }
}

fn to_js<T: Serialize + ?Sized>(value: &T) -> JsValue {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .unwrap_or(JsValue::NULL)
}

fn engine_error(value: JsValue) -> EngineError {
    if let Ok(error) = serde_wasm_bindgen::from_value::<EngineError>(value.clone()) {
        return error;
    }
    let message = Reflect::get(&value, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| value.as_string())
        .unwrap_or_else(|| "ошибка движка git".to_string());
    EngineError::Io { message }
}

fn checked(cancel: &CancelToken) -> Result<(), EngineError> {
    if cancel.is_cancelled() {
        Err(EngineError::Cancelled)
    } else {
        Ok(())
    }
}

impl GitEngine for JsGitEngine {
    async fn probe(&self) -> Result<RepoProbe, EngineError> {
        self.call_as("probe", &[]).await
    }

    async fn init(&self, branch: &str) -> Result<(), EngineError> {
        self.call("init", &[JsValue::from_str(branch)]).await?;
        Ok(())
    }

    async fn head(&self) -> Result<HeadInfo, EngineError> {
        self.call_as("head", &[]).await
    }

    async fn status(&self) -> Result<Vec<RawChange>, EngineError> {
        self.call_as("status", &[]).await
    }

    async fn commit(
        &self,
        paths: &[String],
        message: &str,
        author: &Author,
        time: i64,
        merge_parent: Option<&str>,
    ) -> Result<String, EngineError> {
        self.call_as(
            "commit",
            &[
                to_js(paths),
                JsValue::from_str(message),
                to_js(author),
                JsValue::from_f64(time as f64),
                to_js(&merge_parent),
            ],
        )
        .await
    }

    async fn log(
        &self,
        start: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<LogPage, EngineError> {
        self.call_as(
            "log",
            &[
                to_js(&start),
                JsValue::from_f64(offset as f64),
                JsValue::from_f64(limit as f64),
            ],
        )
        .await
    }

    async fn read_blob(&self, rev: &str, path: &str) -> Result<Option<Vec<u8>>, EngineError> {
        let value = self
            .call(
                "readBlob",
                &[JsValue::from_str(rev), JsValue::from_str(path)],
            )
            .await?;
        if value.is_null() || value.is_undefined() {
            return Ok(None);
        }
        Ok(Some(js_sys::Uint8Array::new(&value).to_vec()))
    }

    async fn tree_diff(&self, from: Option<&str>, to: &str) -> Result<Vec<RawChange>, EngineError> {
        self.call_as("treeDiff", &[to_js(&from), JsValue::from_str(to)])
            .await
    }

    async fn merge_base(&self, a: &str, b: &str) -> Result<Option<String>, EngineError> {
        self.call_as("mergeBase", &[JsValue::from_str(a), JsValue::from_str(b)])
            .await
    }

    async fn set_head(&self, oid: &str) -> Result<(), EngineError> {
        self.call("setHead", &[JsValue::from_str(oid)]).await?;
        Ok(())
    }

    async fn remote_head(&self, branch: &str) -> Result<Option<String>, EngineError> {
        self.call_as("remoteHead", &[JsValue::from_str(branch)])
            .await
    }

    async fn ahead_behind(&self, local: &str, upstream: &str) -> Result<(u32, u32), EngineError> {
        self.call_as(
            "aheadBehind",
            &[JsValue::from_str(local), JsValue::from_str(upstream)],
        )
        .await
    }

    async fn fetch(
        &self,
        remote: &RemoteSpec,
        auth: &Auth,
        cancel: &CancelToken,
    ) -> Result<Option<String>, EngineError> {
        checked(cancel)?;
        let result = self.call_as("fetch", &[to_js(remote), to_js(auth)]).await;
        checked(cancel)?;
        result
    }

    async fn push(
        &self,
        remote: &RemoteSpec,
        auth: &Auth,
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        checked(cancel)?;
        self.call("push", &[to_js(remote), to_js(auth)]).await?;
        Ok(())
    }

    async fn clone_into(
        &self,
        remote: &RemoteSpec,
        auth: &Auth,
        bare: bool,
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        checked(cancel)?;
        self.call(
            "cloneInto",
            &[to_js(remote), to_js(auth), JsValue::from_bool(bare)],
        )
        .await?;
        checked(cancel)
    }

    async fn set_remote_url(&self, url: Option<&str>) -> Result<(), EngineError> {
        self.call("setRemoteUrl", &[to_js(&url)]).await?;
        Ok(())
    }

    async fn global_author(&self) -> Option<Author> {
        // В браузере глобальных настроек git нет.
        None
    }
}
