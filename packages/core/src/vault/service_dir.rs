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

/// Папка внешних шаблонов Typst, положенных пользователем. Едет с
/// синхронизацией: скачать их неоткуда.
pub const PACKAGES_DIR_NAME: &str = "packages";

/// Папка шаблонов, скачанных приложением из сети (фича 041): та же раскладка,
/// что у [`PACKAGES_DIR_NAME`]. Принадлежит устройству и не синхронизируется —
/// каждое устройство скачивает шаблон само.
pub const DOWNLOADS_DIR_NAME: &str = "downloads";

/// Временные файлы атомарной записи (фича 009, research.md §4). Только
/// native: в вебе атомарность даёт сам `FileSystemWritableFileStream`.
///
/// Внутри служебной папки, а не рядом с целью: служебная папка лежит в корне
/// vault, то есть на той же файловой системе, и `rename` оттуда атомарен, а
/// оставшийся после сбоя файл не попадается пользователю на глаза.
pub const TMP_DIR_NAME: &str = "tmp";

/// Снимки заметок (фича 029, research.md §1): опись и тексты снимков.
/// Принадлежат устройству, поэтому входят в производные пути.
pub const SNAPSHOTS_DIR_NAME: &str = "snapshots";

/// Словарь vault для проверки орфографии (фича 030, research.md §4): слово в
/// строке. Не производный путь — едет с синхронизацией, как настройки vault.
pub const DICTIONARY_FILE_NAME: &str = "dictionary.txt";

/// Индекс задач (фича 033, research.md §2): задачи заметок с их отпечатками.
/// Принадлежит устройству, поэтому входит в производные пути.
pub const TASKS_FILE_NAME: &str = "tasks.json";

/// Журналы пакетного импорта (фича 021): по ним прерванный импорт продолжается
/// из папки-источника, которая есть только на этом устройстве. Принадлежат
/// устройству, поэтому входят в производные пути.
pub const IMPORTS_DIR_NAME: &str = "imports";

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

/// Папка снимков заметок относительно корня vault (фича 029).
pub fn snapshots_dir() -> PathBuf {
    service_dir().join(SNAPSHOTS_DIR_NAME)
}

/// Индекс задач относительно корня vault (фича 033).
pub fn tasks_index_path() -> PathBuf {
    service_dir().join(TASKS_FILE_NAME)
}

/// Файл кэша индекса относительно корня vault.
pub fn cache_path() -> PathBuf {
    service_dir().join(CACHE_FILE_NAME)
}

/// Папка внешних шаблонов относительно корня vault.
pub fn packages_dir() -> PathBuf {
    service_dir().join(PACKAGES_DIR_NAME)
}

/// Папка скачанных шаблонов относительно корня vault (фича 041).
pub fn downloads_dir() -> PathBuf {
    service_dir().join(DOWNLOADS_DIR_NAME)
}

/// Файл кэша прежней версии относительно корня vault.
pub fn legacy_cache_path() -> PathBuf {
    PathBuf::from(LEGACY_CACHE_FILE_NAME)
}

/// Производные пути служебного места — то, что восстановимо и принадлежит
/// устройству (фича 011, research §5, §6): кэш индекса вместе с файлами журнала
/// SQLite, временные файлы атомарной записи, снимки заметок, индекс задач,
/// журналы импорта и скачиваемые шаблоны.
///
/// Строки в форме правил исключения, относительно корня vault, через `/`: их
/// читают правила истории и Syncthing, а не файловая система. Настройки vault и
/// локальные шаблоны сюда не входят намеренно — они общие для устройств и
/// обязаны доставляться вместе с заметками; шаблоны — только положенные
/// вручную в пространства имён, которые скачать неоткуда.
pub fn derived_paths() -> Vec<String> {
    vec![
        format!("{SERVICE_DIR_NAME}/{CACHE_FILE_NAME}*"),
        format!("{SERVICE_DIR_NAME}/{TMP_DIR_NAME}"),
        format!("{SERVICE_DIR_NAME}/{SNAPSHOTS_DIR_NAME}"),
        format!("{SERVICE_DIR_NAME}/{TASKS_FILE_NAME}"),
        format!("{SERVICE_DIR_NAME}/{IMPORTS_DIR_NAME}"),
        // Скачиваемые шаблоны (фича 041): всё скачанное, пространство имён
        // реестра Typst — его можно скачать, даже если положено руками, — и
        // перечень скачанного прежних версий.
        format!("{SERVICE_DIR_NAME}/{DOWNLOADS_DIR_NAME}"),
        format!(
            "{SERVICE_DIR_NAME}/{PACKAGES_DIR_NAME}/{}",
            crate::preview::packages::REGISTRY_NAMESPACE
        ),
        format!(
            "{SERVICE_DIR_NAME}/{PACKAGES_DIR_NAME}/{}",
            crate::preview::packages::REGISTRY_FILE
        ),
        // Кэш прежних версий лежал в корне vault (см. LEGACY_CACHE_FILE_NAME):
        // устройство со старой версией ещё может его создавать.
        format!("{LEGACY_CACHE_FILE_NAME}*"),
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
        assert_eq!(
            derived,
            [
                ".typsidian/cache.sqlite*",
                ".typsidian/tmp",
                ".typsidian/snapshots",
                ".typsidian/tasks.json",
                ".typsidian/imports",
                ".typsidian/downloads",
                ".typsidian/packages/preview",
                ".typsidian/packages/installed.json",
                ".typsidian-cache.sqlite*"
            ]
        );
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
