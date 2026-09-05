// Оболочка запускается из библиотеки: `lib.rs` собирается ещё и как cdylib для
// мобильных таргетов, где точки входа `main` не существует.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    typsidian_desktop_lib::run()
}
