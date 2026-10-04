//! Раскладка служебного места vault (фича 006, contracts/vault-settings.md).
//!
//! Единственная папка, в которой приложение хранит что-либо для конкретного
//! vault: кэш индекса, настройки уровня vault, локальные внешние шаблоны
//! (FR-020).
//!
//! # Почему имя начинается с точки
//!
//! Требование «не показывать в дереве» (FR-022) выполняется **само собой**:
//! оба backend уже исключают из обхода записи, начинающиеся с точки —
//! `native::is_excluded` и `web::is_excluded`. Отсюда бесплатно следует и
//! остальное: содержимое не попадает в индекс, значит не участвует ни в
//! подсчёте заметок, ни в графе ссылок, ни в поиске. Имя без точки потребовало
//! бы отдельного правила исключения в двух местах — ровно того расхождения по
//! таргетам, от которого предостерегает принцип II.
//!
//! # Пути относительны корню vault
//!
//! Абсолютных путей в вебе не существует, поэтому контракт их и не требует:
//! всё, что здесь возвращается, передаётся в `VaultBackend`, который сам знает
//! свой корень.

use std::path::{Path, PathBuf};

/// Имя служебной папки внутри vault.
pub const SERVICE_DIR_NAME: &str = ".typsidian";

/// Файл настроек уровня vault.
pub const VAULT_SETTINGS_FILE_NAME: &str = "vault.json";

/// Кэш индекса. Только native: в вебе кэш живёт в памяти
/// (`vault::cache::MemoryCache`), файла там никогда не было.
pub const CACHE_FILE_NAME: &str = "cache.sqlite";

/// Папка локальных внешних шаблонов Typst.
pub const PACKAGES_DIR_NAME: &str = "packages";

/// Временные файлы атомарной записи (фича 009, research.md §4). Только
/// native: в вебе атомарность даёт сам `FileSystemWritableFileStream`.
///
/// Внутри служебной папки, а не рядом с целью: служебная папка лежит в корне
/// vault, то есть на той же файловой системе, и `rename` оттуда атомарен, а
/// оставшийся после сбоя файл не попадается пользователю на глаза.
pub const TMP_DIR_NAME: &str = "tmp";

/// Где кэш индекса лежал до фичи 006 — в корне vault, отдельным файлом.
///
/// Существует только ради уборки: при первом открытии vault новой версией файл
/// удаляется, а кэш строится заново (FR-021, research.md §11). Переносить его
/// содержимое незачем — кэш производен и полностью восстановим, а код миграции
/// схемы SQLite понадобился бы ровно один раз и остался бы навсегда.
pub const LEGACY_CACHE_FILE_NAME: &str = ".typsidian-cache.sqlite";

/// Служебная папка относительно корня vault.
pub fn service_dir() -> PathBuf {
    PathBuf::from(SERVICE_DIR_NAME)
}

/// Файл настроек уровня vault относительно корня vault.
pub fn vault_settings_path() -> PathBuf {
    service_dir().join(VAULT_SETTINGS_FILE_NAME)
}

/// Папка временных файлов записи относительно корня vault.
pub fn tmp_dir() -> PathBuf {
    service_dir().join(TMP_DIR_NAME)
}

/// Файл кэша индекса относительно корня vault.
pub fn cache_path() -> PathBuf {
    service_dir().join(CACHE_FILE_NAME)
}

/// Папка внешних шаблонов относительно корня vault.
pub fn packages_dir() -> PathBuf {
    service_dir().join(PACKAGES_DIR_NAME)
}

/// Файл кэша прежней версии относительно корня vault.
pub fn legacy_cache_path() -> PathBuf {
    PathBuf::from(LEGACY_CACHE_FILE_NAME)
}

/// Производные пути служебного места — то, что восстановимо и принадлежит
/// устройству (фича 011, research §5, §6): кэш индекса вместе с файлами журнала
/// SQLite и временные файлы атомарной записи.
///
/// Строки в форме правил исключения, относительно корня vault, через `/`: их
/// читают правила истории и Syncthing, а не файловая система. Настройки vault и
/// локальные шаблоны сюда не входят намеренно — они общие для устройств и
/// обязаны доставляться вместе с заметками.
pub fn derived_paths() -> Vec<String> {
    vec![
        format!("{SERVICE_DIR_NAME}/{CACHE_FILE_NAME}*"),
        format!("{SERVICE_DIR_NAME}/{TMP_DIR_NAME}"),
    ]
}

/// Лежит ли путь внутри служебного места.
///
/// Сравнивается **первый компонент**, а не любой: служебное место у vault одно
/// и лежит в его корне. Папка `Проекты/.typsidian/`, если пользователь такую
/// заведёт, нашей не является — и в дерево всё равно не попадёт, потому что
/// начинается с точки.
pub fn is_inside_service_dir(path: &Path) -> bool {
    path.components()
        .next()
        .and_then(|component| component.as_os_str().to_str())
        .is_some_and(|name| name == SERVICE_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_paths_cover_caches_but_not_settings() {
        let derived = derived_paths();
        assert_eq!(derived, [".typsidian/cache.sqlite*", ".typsidian/tmp"]);
        assert!(!derived
            .iter()
            .any(|path| path.contains(VAULT_SETTINGS_FILE_NAME)));
    }

    #[test]
    fn layout_matches_the_contract() {
        assert_eq!(service_dir(), Path::new(".typsidian"));
        assert_eq!(vault_settings_path(), Path::new(".typsidian/vault.json"));
        assert_eq!(cache_path(), Path::new(".typsidian/cache.sqlite"));
        assert_eq!(packages_dir(), Path::new(".typsidian/packages"));
    }

    #[test]
    fn every_path_is_relative_to_the_vault_root() {
        // Абсолютного пути в вебе не существует: контракт не имеет права его
        // требовать ни на одном таргете.
        for path in [
            service_dir(),
            vault_settings_path(),
            cache_path(),
            packages_dir(),
            legacy_cache_path(),
        ] {
            assert!(
                path.is_relative(),
                "{} обязан быть относительным",
                path.display()
            );
        }
    }

    #[test]
    fn membership_is_decided_by_the_first_component() {
        assert!(is_inside_service_dir(Path::new(".typsidian/vault.json")));
        assert!(is_inside_service_dir(Path::new(
            ".typsidian/packages/preview/codly/1.3.0/lib.typ"
        )));
        assert!(is_inside_service_dir(Path::new(".typsidian")));

        assert!(!is_inside_service_dir(Path::new("Идея.typ")));
        assert!(!is_inside_service_dir(Path::new(
            "Проекты/.typsidian/x.json"
        )));
        // Похожее имя служебной папкой не является.
        assert!(!is_inside_service_dir(Path::new(".typsidian-cache.sqlite")));
    }
}
