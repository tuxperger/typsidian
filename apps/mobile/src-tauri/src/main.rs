// На Android этот файл не участвует: приложение стартует из `MainActivity`, а
// в библиотеку приходит через JNI. Он нужен, чтобы крейт можно было собрать и
// проверить нативно — иначе ошибки в общих командах ловились бы только полной
// сборкой APK.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    typsidian_mobile_lib::run()
}
