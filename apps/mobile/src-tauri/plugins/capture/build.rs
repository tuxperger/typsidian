/// Команды плагина. `register_listener` и `remove_listener` нужны JS-функции
/// `addPluginListener`: подписка на события идёт через них.
const COMMANDS: &[&str] = &[
    "pending",
    "resolve",
    "return_to_source",
    "image",
    "register_listener",
    "remove_listener",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
