//! wasm-биндинги быстрого захвата (contracts/core-capture-api.md).
//!
//! Тонкие обёртки: правил здесь нет. Чистые правила — свободные функции;
//! всё, что читает или пишет vault, — методы `WebBackend` с теми же именами,
//! что у команд Tauri.

use wasm_bindgen::prelude::*;

use super::{
    capture_settings, capture_target_path, capture_write, daily_date_of, daily_neighbours,
    daily_path, entry, open_daily, Capture, CaptureOutcome, CaptureSettings, DailyDate, DailyError,
    DailyNeighbours, OpenDaily,
};
use crate::vault::vault_settings::{read_vault_settings, VaultOverrides};
use crate::vault::web::WebBackend;

/// Настройки захвата с умолчаниями из настроек vault.
#[wasm_bindgen(js_name = captureSettings)]
pub fn capture_settings_js(overrides: VaultOverrides) -> CaptureSettings {
    capture_settings(overrides.capture.as_ref())
}

/// Запись захвата разметкой Typst — образец в настройках и сама запись.
/// `imagePath` — путь вставки картинки; без него подставляется имя файла.
#[wasm_bindgen(js_name = captureEntry)]
pub fn capture_entry_js(
    settings: CaptureSettings,
    capture: Capture,
    image_path: Option<String>,
) -> String {
    entry(&settings, &capture, image_path.as_deref())
}

/// Путь заметки назначения; `undefined` — назначение не заметка vault.
#[wasm_bindgen(js_name = captureTargetPath)]
pub fn capture_target_path_js(settings: CaptureSettings, capture: Capture) -> Option<String> {
    capture_target_path(&settings, &capture)
}

/// Путь ежедневной заметки на дату; `undefined` — такого дня нет.
#[wasm_bindgen(js_name = dailyPath)]
pub fn daily_path_js(settings: CaptureSettings, date: DailyDate) -> Option<String> {
    daily_path(&settings, &date)
}

/// Дата заметки, если это ежедневная заметка; иначе `undefined`.
#[wasm_bindgen(js_name = dailyDateOf)]
pub fn daily_date_of_js(settings: CaptureSettings, path: &str) -> Option<DailyDate> {
    daily_date_of(&settings, path)
}

#[wasm_bindgen]
impl WebBackend {
    /// Дописывает захват в заметку назначения (§6). `openBuffer` — полный
    /// текст заметки в редакторе, если она там открыта; `image` — байты
    /// картинки для захвата изображения.
    #[wasm_bindgen(js_name = captureWrite)]
    pub async fn capture_write_js(
        &mut self,
        capture: Capture,
        open_buffer: Option<String>,
        image: Option<Vec<u8>>,
    ) -> CaptureOutcome {
        capture_write(self, &capture, open_buffer.as_deref(), image.as_deref()).await
    }

    /// «Сегодня»: путь ежедневной заметки на дату, созданной при отсутствии.
    #[wasm_bindgen(js_name = openDaily)]
    pub async fn open_daily_js(&mut self, date: DailyDate) -> Result<OpenDaily, DailyError> {
        let overrides = read_vault_settings(self).await.unwrap_or_default();
        let settings = capture_settings(overrides.capture.as_ref());
        open_daily(self, &settings, &date).await
    }

    /// Ближайшие существующие ежедневные заметки раньше и позже даты.
    #[wasm_bindgen(js_name = dailyNeighbours)]
    pub async fn daily_neighbours_js(&self, date: DailyDate) -> DailyNeighbours {
        let overrides = read_vault_settings(self).await.unwrap_or_default();
        let settings = capture_settings(overrides.capture.as_ref());
        daily_neighbours(self, &settings, &date).await
    }
}
