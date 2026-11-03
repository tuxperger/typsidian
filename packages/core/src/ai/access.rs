//! Единая проверка доступа агента к файлам (FR-015, FR-016): только внутри vault,
//! служебное место закрыто всегда, запрещённые папки — со всем вложенным.
//! Сравнение по компонентам: `Личное2/` не попадает под запрет `Личное/`.
//!
//! Компоненты сравниваются так, как их сравнивает самая снисходительная файловая
//! система, куда vault может попасть: без учёта регистра (macOS, Windows, память
//! Android), Unicode-формы (HFS+ хранит NFD) и хвостовых точек и пробелов
//! (Windows открывает `Личное.` как `Личное`). Иначе другое написание того же
//! пути прошло бы мимо запрета (ревью безопасности 050, M4).

use std::path::{Component, Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

use super::settings::{load_settings, save_settings, AiSettings};
use crate::vault::backend::VaultBackend;
use crate::vault::service_dir::SERVICE_DIR_NAME;

/// Папка версий Syncthing в корне vault.
const VERSIONS_DIR_NAME: &str = ".stversions";

/// Относительный путь внутри vault без `.` и `..`; иначе `None`.
fn clean(path: &Path) -> Option<PathBuf> {
    let mut parts = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.as_os_str().is_empty()).then_some(parts)
}

pub(super) fn normalize_folder(folder: &Path) -> Option<String> {
    clean(folder).map(|path| path.to_string_lossy().replace('\\', "/"))
}

/// Компоненты пути в форме для сравнения.
pub(super) fn folded(path: &Path) -> Option<Vec<String>> {
    let path = clean(path)?;
    let parts: Vec<String> = path
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_string_lossy()
                .trim_end_matches(['.', ' '])
                .nfc()
                .collect::<String>()
                .to_lowercase()
        })
        .collect();
    // `...` и `. ` после обрезки пусты: Windows понимает их как текущую папку.
    (!parts.iter().any(String::is_empty)).then_some(parts)
}

/// Проверка по одной строке пути. Выключенная функция не даёт доступа ни к чему.
pub fn check_access(path: &str, settings: &AiSettings) -> bool {
    if !settings.enabled {
        return false;
    }
    let Some(path) = folded(Path::new(path)) else {
        return false;
    };
    // Служебное место приложения и история версий Syncthing: в ней лежат старые копии
    // заметок, в том числе из запрещённых папок и уже удалённые (ревью безопасности 051, L9).
    if path
        .first()
        .is_some_and(|first| first == SERVICE_DIR_NAME || first == VERSIONS_DIR_NAME)
    {
        return false;
    }
    !settings
        .denied_folders
        .iter()
        .filter_map(|folder| folded(Path::new(folder)))
        .any(|folder| path.starts_with(&folder))
}

/// Проверка с файловой системой: путь, идущий через символическую ссылку или
/// сам ею являющийся, закрыт — ссылка увела бы чтение в запрещённую папку или за
/// пределы vault, а по строке пути этого не видно.
pub async fn check_access_in<B: VaultBackend>(
    backend: &B,
    path: &str,
    settings: &AiSettings,
) -> bool {
    if !check_access(path, settings) {
        return false;
    }
    let path = Path::new(path);
    !backend.crosses_symlink(path).await && !backend.is_symlink(path).await
}

/// Запрет едет за папкой при переименовании и переносе в приложении.
pub async fn move_denied_folders<B: VaultBackend>(backend: &mut B, from: &Path, to: &Path) {
    let (Some(from), Some(to)) = (folded(from), clean(to)) else {
        return;
    };
    let mut settings = load_settings(backend).await;
    let mut changed = false;
    for folder in &mut settings.denied_folders {
        let (Some(current), Some(original)) = (
            folded(Path::new(folder.as_str())),
            clean(Path::new(folder.as_str())),
        ) else {
            continue;
        };
        if current.starts_with(&from) {
            let rest: PathBuf = original.components().skip(from.len()).collect();
            let moved = if rest.as_os_str().is_empty() {
                to.clone()
            } else {
                to.join(rest)
            };
            *folder = moved.to_string_lossy().replace('\\', "/");
            changed = true;
        }
    }
    if changed {
        let _ = save_settings(backend, &settings).await;
    }
}
