//! Последний открытый vault на нативных клиентах (FR-007 фичи 001).
//!
//! Проверяется то, что делают команды `remember_vault`/`last_vault`, без окна:
//! сами команды добавляют к этим функциям ровно получение хранилища из
//! состояния окна. Ради этого тела команд и вынесены в
//! [`typsidian_shell::remember_opened_vault`] и
//! [`typsidian_shell::last_opened_vault`] — иначе «десктоп не помнит последний
//! vault» проверялось бы только запуском приложения и разглядыванием файла
//! настроек.
//!
//! Правила, которые тут видны (тождество папки по канонизированному пути,
//! запрет розыска перенесённой папки), живут в ядре и проверены его тестами —
//! здесь они появляются как следствие, а не как вторая реализация.

use std::fs;

use tempfile::TempDir;
use typsidian_core::settings::native::NativeStore;
use typsidian_core::settings::store::SettingsStore;
use typsidian_shell::{last_opened_vault, remember_opened_vault};

/// Хранилище настроек в отдельном каталоге: настоящий каталог конфигурации
/// пользователя тесты не трогают.
async fn store_in(dir: &TempDir) -> NativeStore {
    NativeStore::open_in(dir.path().to_path_buf())
        .await
        .expect("открытие хранилища настроек")
}

#[tokio::test]
async fn an_opened_folder_becomes_the_last_vault_and_survives_a_restart() {
    let config = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();

    {
        let mut store = store_in(&config).await;
        assert!(
            last_opened_vault(&store).await.is_none(),
            "пока ничего не открывали, предлагать нечего"
        );
        remember_opened_vault(&mut store, vault.path().to_path_buf())
            .await
            .expect("запоминание открытой папки");
    }

    // Перезапуск приложения: хранилище открывается заново.
    let store = store_in(&config).await;
    let last = last_opened_vault(&store)
        .await
        .expect("последний vault обязан пережить перезапуск");
    assert_eq!(
        fs::canonicalize(last.path).unwrap(),
        fs::canonicalize(vault.path()).unwrap(),
        "открывать предлагается та же самая папка"
    );
}

#[tokio::test]
async fn opening_the_same_folder_twice_does_not_add_a_second_record() {
    let config = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();

    let mut store = store_in(&config).await;
    remember_opened_vault(&mut store, vault.path().to_path_buf())
        .await
        .unwrap();
    remember_opened_vault(&mut store, vault.path().to_path_buf())
        .await
        .unwrap();

    assert_eq!(
        SettingsStore::settings(&store).known_vaults.len(),
        1,
        "та же папка переиспользует запись (FR-020 фичи 004)"
    );
}

#[tokio::test]
async fn a_vanished_folder_is_not_offered() {
    let config = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();
    let path = vault.path().to_path_buf();

    let mut store = store_in(&config).await;
    remember_opened_vault(&mut store, path).await.unwrap();

    // Папки не стало между запусками: розыск запрещён (FR-023 фичи 004), и
    // открывать нечего — окно покажет обычный экран приветствия.
    drop(vault);

    assert!(last_opened_vault(&store).await.is_none());
}

#[tokio::test]
async fn the_newest_opened_folder_replaces_the_previous_one() {
    let config = TempDir::new().unwrap();
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();

    let mut store = store_in(&config).await;
    remember_opened_vault(&mut store, first.path().to_path_buf())
        .await
        .unwrap();
    remember_opened_vault(&mut store, second.path().to_path_buf())
        .await
        .unwrap();

    let last = last_opened_vault(&store).await.expect("последний vault");
    assert_eq!(
        fs::canonicalize(last.path).unwrap(),
        fs::canonicalize(second.path()).unwrap()
    );
    assert_eq!(
        SettingsStore::settings(&store).known_vaults.len(),
        2,
        "прежний vault остаётся известным, перестав быть текущим"
    );
}
