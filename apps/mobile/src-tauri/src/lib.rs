//! Мобильная оболочка (Android).
//!
//! Всё содержательное — команды поверх `typsidian-core` — живёт в
//! `packages/shell` и общее с рабочим столом. Здесь остаётся ровно то, что у
//! каждой оболочки своё: раскрытие `generate_context!()`, а с ним свой
//! `tauri.conf.json` (идентификатор `dev.typsidian.mobile`, он же имя пакета
//! Android), свои иконки и свои capabilities.
//!
//! Право писать доски эта сборка не получает (фича 002, FR-034/FR-035), и
//! отдельной настройки для этого не заводится: `typsidian_shell` выбирает
//! профиль по `target_os`, так что ограничение приезжает самим фактом сборки
//! под Android.

/// Точка входа оболочки.
///
/// Единственный плагин — локальный `capture` (фича 022): приём «Поделиться» и
/// ярлыков Android. `tauri-plugin-dialog` отсутствует намеренно: его
/// `pick_folder` реализован только под desktop, а папку vault на Android
/// выбирают обзором в самом окне поверх команд `browse_*`.
///
/// `main` здесь не участвует: приложение Android стартует из `MainActivity` и
/// приходит сюда через JNI. Символ, который ищет загрузчик, создаёт
/// `mobile_entry_point`.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    typsidian_shell::builder()
        .plugin(tauri_plugin_capture::init())
        .run(tauri::generate_context!())
        .expect("оболочка Typsidian должна запуститься");
}
