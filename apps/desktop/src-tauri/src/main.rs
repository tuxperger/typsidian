// Оболочка запускается из библиотеки: так `run()` остаётся одной строкой и на
// рабочем столе, и в мобильном крейте, где точки входа `main` не существует
// вовсе и вместо неё JNI зовёт `cdylib`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    typsidian_desktop_lib::run()
}
