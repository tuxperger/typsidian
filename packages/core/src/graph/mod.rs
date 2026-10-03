//! Смежный модуль ядра: минимальное извлечение ссылок между заметками,
//! достаточное для FR-026…FR-031 (research.md §8). Полная семантика графа ссылок
//! (парсинг всех форм Typst-ссылок) — предмет отдельной будущей фичи; здесь —
//! узкий парсер под нужды vault-manager: ссылки вида `[[путь]]`.
//!
//! Граница ответственности (research.md §8): парсинг и разрешение целей — здесь,
//! хранение таблицы `links` и запросы обратных ссылок — в `vault::cache`.
//! Vault-manager потребляет обратные ссылки, но не является их автором.

use std::path::{Component, Path, PathBuf};

/// Расширение файла заметки. Ссылка без расширения трактуется как ссылка на
/// заметку — писать `[[заметка.typ]]` от пользователя не требуется.
const NOTE_EXTENSION: &str = "typ";

/// Извлекает пути, на которые ссылается содержимое заметки.
pub fn extract_links(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let bytes = content.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(end) = content[i + 2..].find("]]") {
                let target = content[i + 2..i + 2 + end].trim();
                if !target.is_empty() {
                    links.push(target.to_string());
                }
                i += 2 + end + 2;
                continue;
            }
        }
        i += 1;
    }
    links
}

/// Схлопывает `.` и `..` в пути, не обращаясь к файловой системе: цели ссылок
/// разрешаются относительно корня vault и вполне могут указывать на ещё не
/// существующую (битую) заметку, поэтому `canonicalize` здесь неприменим.
/// Выход за пределы корня отбрасывается — ссылка наружу vault для целей
/// FR-026…FR-031 неотличима от битой.
fn normalize(relative: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::Normal(part) => out.push(part),
            // Корень/префикс внутри vault-относительного пути смысла не имеет.
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Разрешает сырую цель ссылки в путь заметки относительно корня vault.
///
/// - `[[имя]]` — соседняя заметка, относительно папки самой `source`;
/// - `[[/путь/имя]]` — от корня vault;
/// - расширение `.typ` дописывается, если его нет.
///
/// `None`, если цель пуста или уводит за пределы vault. Существование цели
/// здесь не проверяется: битая ссылка — нормальный результат (FR-030),
/// определяемый уже по индексу заметок.
pub fn resolve_link_target(source: &Path, raw: &str) -> Option<PathBuf> {
    let raw = raw.trim().replace('\\', "/");
    if raw.is_empty() {
        return None;
    }

    let (rooted, rest) = match raw.strip_prefix('/') {
        Some(rest) => (true, rest.to_string()),
        None => (false, raw),
    };
    if rest.is_empty() {
        return None;
    }

    let mut target = PathBuf::from(&rest);
    if target.extension().and_then(|e| e.to_str()) != Some(NOTE_EXTENSION) {
        let file_name = format!("{}.{NOTE_EXTENSION}", target.file_name()?.to_string_lossy());
        target.set_file_name(file_name);
    }

    if rooted {
        normalize(&target)
    } else {
        let base = source.parent().unwrap_or_else(|| Path::new(""));
        normalize(&base.join(target))
    }
}

/// Исходящие ссылки заметки `source` как пути относительно корня vault.
/// Порядок первого появления сохраняется, дубли схлопываются: таблица `links`
/// хранит факт связи, а не число упоминаний.
pub fn links_in(source: &Path, content: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for raw in extract_links(content) {
        if let Some(target) = resolve_link_target(source, &raw) {
            // Ссылка заметки на саму себя битой стать не может и в списке
            // «что сломается при удалении» только шумит.
            if target == source {
                continue;
            }
            if !out.contains(&target) {
                out.push(target);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_simple_link() {
        let content = "смотри [[другая заметка]] и текст";
        assert_eq!(extract_links(content), vec!["другая заметка"]);
    }

    #[test]
    fn extracts_multiple_links() {
        let content = "[[a]] потом [[b]]";
        assert_eq!(extract_links(content), vec!["a", "b"]);
    }

    #[test]
    fn no_links_in_plain_text() {
        assert!(extract_links("просто текст без ссылок").is_empty());
    }

    #[test]
    fn link_without_extension_resolves_to_note_next_to_source() {
        assert_eq!(
            resolve_link_target(Path::new("sub/source.typ"), "target"),
            Some(PathBuf::from("sub/target.typ"))
        );
    }

    #[test]
    fn link_with_explicit_extension_is_not_doubled() {
        assert_eq!(
            resolve_link_target(Path::new("source.typ"), "target.typ"),
            Some(PathBuf::from("target.typ"))
        );
    }

    #[test]
    fn leading_slash_resolves_from_vault_root() {
        assert_eq!(
            resolve_link_target(Path::new("deep/nested/source.typ"), "/top"),
            Some(PathBuf::from("top.typ"))
        );
    }

    #[test]
    fn parent_segments_are_collapsed_without_touching_disk() {
        assert_eq!(
            resolve_link_target(Path::new("a/b/source.typ"), "../sibling"),
            Some(PathBuf::from("a/sibling.typ"))
        );
    }

    #[test]
    fn link_escaping_vault_root_is_dropped() {
        assert_eq!(
            resolve_link_target(Path::new("source.typ"), "../outside"),
            None
        );
        assert_eq!(resolve_link_target(Path::new("source.typ"), "   "), None);
    }

    #[test]
    fn links_in_deduplicates_and_skips_self_reference() {
        let links = links_in(
            Path::new("notes/source.typ"),
            "[[target]] снова [[target]] и [[source]]",
        );
        assert_eq!(links, vec![PathBuf::from("notes/target.typ")]);
    }
}
