//! Плагин `capture`: приём «Поделиться» и ярлыков приложения на Android (фича 022).
//!
//! Вся работа — в Kotlin-части (`android/`): она разбирает `ACTION_SEND` и
//! `ACTION_VIEW`, кладёт захваты в очередь `filesDir/captures/` и шлёт события
//! `shared` и `shortcut`. Здесь — только проводка команд в неё. Формат записи
//! очереди — это `Capture` ядра (`packages/core/src/capture`), и Rust его не
//! разбирает: значения идут как есть.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{
    command,
    ipc::Channel,
    plugin::{Builder, TauriPlugin},
    AppHandle, Runtime,
};

/// Ответ `pending`: Kotlin не умеет вернуть голый массив, поэтому он завёрнут.
#[derive(Deserialize)]
struct Pending {
    captures: Vec<Value>,
}

/// Ответ `image`: байты картинки в base64 (одной строкой, без переводов строк).
#[derive(Deserialize)]
struct ImageData {
    data: String,
}

#[derive(Serialize)]
struct IdArgs {
    id: String,
}

#[derive(Serialize)]
struct FileArgs {
    file: String,
}

#[derive(Serialize)]
struct RegisterListenerArgs {
    event: String,
    handler: Channel<Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoveListenerArgs {
    event: String,
    channel_id: u32,
}

/// Дескриптор Kotlin-плагина; живёт в состоянии приложения.
#[cfg(target_os = "android")]
struct Native<R: Runtime>(tauri::plugin::PluginHandle<R>);

/// Вызов команды Kotlin-части.
#[cfg(target_os = "android")]
fn call<R: Runtime, A: Serialize, T: serde::de::DeserializeOwned>(
    app: &AppHandle<R>,
    command: &str,
    args: A,
) -> Result<T, String> {
    use tauri::Manager;
    app.state::<Native<R>>()
        .0
        .run_mobile_plugin(command, args)
        .map_err(|e| e.to_string())
}

/// Вне Android принимать «Поделиться» нечему: плагин подключён, но отвечает отказом.
#[cfg(not(target_os = "android"))]
fn call<R: Runtime, A: Serialize, T: serde::de::DeserializeOwned>(
    _app: &AppHandle<R>,
    _command: &str,
    _args: A,
) -> Result<T, String> {
    Err("плагин capture работает только на Android".to_owned())
}

/// Ждущие захваты в порядке поступления.
#[command]
fn pending<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Value>, String> {
    call::<R, _, Pending>(&app, "pending", ()).map(|p| p.captures)
}

/// Убрать запись очереди (и файл картинки) после записи в vault либо отказа.
#[command]
fn resolve<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    call(&app, "resolve", IdArgs { id })
}

/// `moveTaskToBack`, если окно открыто из «Поделиться».
#[command]
fn return_to_source<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    call(&app, "returnToSource", ())
}

/// Байты картинки захвата (base64) по `content.file`.
#[command]
fn image<R: Runtime>(app: AppHandle<R>, file: String) -> Result<String, String> {
    call::<R, _, ImageData>(&app, "image", FileArgs { file }).map(|i| i.data)
}

/// Подписка на события плагина (`addPluginListener`).
#[command]
fn register_listener<R: Runtime>(
    app: AppHandle<R>,
    event: String,
    handler: Channel<Value>,
) -> Result<(), String> {
    call(
        &app,
        "registerListener",
        RegisterListenerArgs { event, handler },
    )
}

/// Снятие подписки.
#[command]
fn remove_listener<R: Runtime>(
    app: AppHandle<R>,
    event: String,
    channel_id: u32,
) -> Result<(), String> {
    call(
        &app,
        "removeListener",
        RemoveListenerArgs { event, channel_id },
    )
}

/// Подключение плагина: `.plugin(tauri_plugin_capture::init())`.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("capture")
        .invoke_handler(tauri::generate_handler![
            pending,
            resolve,
            return_to_source,
            image,
            register_listener,
            remove_listener
        ])
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle =
                    _api.register_android_plugin("dev.typsidian.capture", "CapturePlugin")?;
                _app.manage(Native(handle));
            }
            Ok(())
        })
        .build()
}
