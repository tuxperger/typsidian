//! Смежный модуль ядра: минимальное извлечение ссылок между заметками,
//! достаточное для FR-026…FR-031 (research.md §8). Полная семантика графа ссылок
//! (парсинг всех форм Typst-ссылок) — предмет отдельной будущей фичи; здесь —
//! узкий парсер под нужды vault-manager: ссылки вида `[[путь]]`.
//!
//! Граница ответственности (research.md §8): парсинг и разрешение целей — здесь,
//! хранение таблицы `links` и запросы обратных ссылок — в `vault::cache`.
//! Vault-manager потребляет обратные ссылки, но не является их автором.

use std::path::{Component, Path, PathBuf};

pub mod view;

#[cfg(target_arch = "wasm32")]
pub mod web;

/// Расширение файла заметки. Ссылка без расширения трактуется как ссылка на
/// заметку — писать `[[заметка.typ]]` от пользователя не требуется.
const NOTE_EXTENSION: &str = "typ";

/// Место ссылки `[[…]]` в тексте заметки (фича 013, research.md §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSite {
    /// Цель как написана, без скобок и без `|текста`, обрезанная по пробелам.
    /// Всё, что разбирает ссылки (индекс, связи, граф, перенос), видит цель
    /// только по этому полю (фича 021, research.md §10).
    pub raw: String,
    /// Отображаемый текст из записи `[[путь|текст]]`; `None`, если `|` нет или
    /// после неё пусто.
    pub label: Option<String>,
    /// Диапазон всей записи вместе со скобками `[[…]]`, в байтах текста. У
    /// встраивания — путь заметки внутри кавычек, без `#раздела`.
    pub byte_start: usize,
    pub byte_end: usize,
    /// Встраивание `#embed("…")` (фича 031): весь вызов, с `#`. У `[[…]]` —
    /// `None`.
    pub call: Option<(usize, usize)>,
}

/// Все ссылки заметки с их местами, в порядке следования в тексте.
///
/// Единственный разбор `[[…]]` в ядре: индекс ссылок и панель связей обязаны
/// видеть одни и те же ссылки, поэтому [`extract_links`] — отображение этой
/// функции. Запись без закрывающей `]]` и с пустой целью ссылкой не считается.
/// Запись `[[путь|текст]]` делится по первой `|`: цель — `raw`, остаток —
/// `label`.
pub fn link_sites(content: &str) -> Vec<LinkSite> {
    let mut sites = Vec::new();
    let bytes = content.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(end) = content[i + 2..].find("]]") {
                let inner = &content[i + 2..i + 2 + end];
                let (target, label) = match inner.split_once('|') {
                    Some((target, label)) => {
                        let label = label.trim();
                        (
                            target.trim(),
                            (!label.is_empty()).then(|| label.to_string()),
                        )
                    }
                    None => (inner.trim(), None),
                };
                let byte_end = i + 2 + end + 2;
                if !target.is_empty() {
                    sites.push(LinkSite {
                        raw: target.to_string(),
                        label,
                        byte_start: i,
                        byte_end,
                        call: None,
                    });
                }
                i = byte_end;
                continue;
            }
        }
        i += 1;
    }
    // Встраивания — тоже ссылки (031, research.md §4): индекс, связи, граф,
    // перенос и «Состояние ссылок» видят их без своего разбора.
    for call in crate::embeds::calls(content) {
        let note_end = call.raw.find('#').unwrap_or(call.raw.len());
        let note = &call.raw[..note_end];
        let trimmed = note.trim();
        if trimmed.is_empty() {
            continue;
        }
        let start = call.string.start + (note.len() - note.trim_start().len());
        sites.push(LinkSite {
            raw: trimmed.to_owned(),
            label: None,
            byte_start: start,
            byte_end: start + trimmed.len(),
            call: Some((call.call.start, call.call.end)),
        });
    }
    sites.sort_by_key(|site| site.byte_start);
    sites
}

/// Извлекает пути, на которые ссылается содержимое заметки.
pub fn extract_links(content: &str) -> Vec<String> {
    link_sites(content)
        .into_iter()
        .map(|site| site.raw)
        .collect()
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
/// - расширение `.typ` дописывается, если его нет; расширение доски
///   (`.excalidraw`) сохраняется как есть (фича 013, research.md §3).
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
    let extension = target.extension().and_then(|e| e.to_str());
    if extension != Some(NOTE_EXTENSION) && extension != Some(crate::vault::BOARD_EXTENSION) {
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

/// Самая короткая запись ссылки `[[…]]`, которая из папки `source`
/// разрешается ровно в `target` (фича 013, FR-017, research.md §9).
///
/// - цель в той же папке → `[[название]]`, иначе `[[/путь/название]]`;
/// - `.typ` опускается, `.excalidraw` остаётся;
/// - результат проверяется обратным разбором: `link_sites` обязан увидеть в нём
///   одну ссылку, а [`resolve_link_target`] — вернуть `target`. Путь с `]]`,
///   краевыми пробелами или обратной косой чертой этой проверки не проходит, и
///   тогда записать ссылку на заметку нельзя — `None`.
pub fn link_text(source: &Path, target: &Path) -> Option<String> {
    let full = target.to_str()?.replace('\\', "/");
    let shown = full
        .strip_suffix(&format!(".{NOTE_EXTENSION}"))
        .unwrap_or(&full);
    let same_folder = source.parent().unwrap_or_else(|| Path::new(""))
        == target.parent().unwrap_or_else(|| Path::new(""));
    let inner = if same_folder {
        let name = target.file_name()?.to_str()?;
        name.strip_suffix(&format!(".{NOTE_EXTENSION}"))
            .unwrap_or(name)
            .to_string()
    } else {
        format!("/{shown}")
    };
    let text = format!("[[{inner}]]");

    let sites = link_sites(&text);
    let [site] = sites.as_slice() else {
        return None;
    };
    if site.raw != inner || site.byte_start != 0 || site.byte_end != text.len() {
        return None;
    }
    (resolve_link_target(source, &inner).as_deref() == Some(target)).then_some(text)
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

/// Файлы, вставленные в заметку вызовом `#image("…")` с литеральным путём
/// (фича 025, research.md §3): картинки и снимки досок 002.
///
/// Путь разрешается как в Typst: без `/` в начале — от папки заметки, с `/` —
/// от корня vault. Разбор — парсером Typst, как в `elements::locate`: он
/// знает скобки внутри строк и вложенность в `#figure(image(…))`. Вызов не с
/// литералом (`image(bytes)`, переменная) и адрес с `://` пропускаются.
/// Порядок первого появления сохраняется, дубли схлопываются.
pub fn embeds_in(source: &Path, content: &str) -> Vec<PathBuf> {
    use typst::syntax::{ast, parse, SyntaxKind, SyntaxNode};

    // Дешёвый отсев: большинство заметок картинок не вставляют, а разбор
    // Typst на каждую заметку при пересчёте vault не бесплатен.
    if !content.contains("image") {
        return Vec::new();
    }
    let root = parse(content);
    let mut out: Vec<PathBuf> = Vec::new();
    let mut stack: Vec<&SyntaxNode> = vec![&root];
    while let Some(node) = stack.pop() {
        if node.kind() == SyntaxKind::FuncCall {
            let mut children = node.children();
            let is_image = children
                .next()
                .and_then(|callee| callee.cast::<ast::Ident>())
                .is_some_and(|name| name.get() == "image");
            let literal = is_image
                .then(|| children.find(|child| child.kind() == SyntaxKind::Args))
                .flatten()
                .and_then(|args| args.children().find(|arg| arg.kind() == SyntaxKind::Str))
                .and_then(|arg| arg.cast::<ast::Str>())
                .map(|text| text.get().to_string());
            if let Some(target) = literal.and_then(|raw| resolve_embed_target(source, &raw)) {
                if !out.contains(&target) {
                    out.push(target);
                }
            }
        }
        // Обход в глубину: дети кладутся в обратном порядке, чтобы порядок
        // появления в тексте сохранился.
        let before = stack.len();
        stack.extend(node.children());
        stack[before..].reverse();
    }
    out
}

/// Вставки файлов с местами (фича 028): цель и байтовый диапазон вызова
/// `image(…)` (без `#` перед ним), каждая запись по порядку текста, без
/// схлопывания дублей — починке нужна каждая. Разбор тот же, что у
/// [`embeds_in`].
pub fn embed_sites(source: &Path, content: &str) -> Vec<(PathBuf, usize, usize)> {
    use typst::syntax::{ast, parse, LinkedNode, SyntaxKind};

    if !content.contains("image") {
        return Vec::new();
    }
    let root = parse(content);
    let mut out = Vec::new();
    let mut stack: Vec<LinkedNode> = vec![LinkedNode::new(&root)];
    while let Some(node) = stack.pop() {
        if node.kind() == SyntaxKind::FuncCall {
            let mut children = node.get().children();
            let is_image = children
                .next()
                .and_then(|callee| callee.cast::<ast::Ident>())
                .is_some_and(|name| name.get() == "image");
            let literal = is_image
                .then(|| children.find(|child| child.kind() == SyntaxKind::Args))
                .flatten()
                .and_then(|args| args.children().find(|arg| arg.kind() == SyntaxKind::Str))
                .and_then(|arg| arg.cast::<ast::Str>())
                .map(|text| text.get().to_string());
            if let Some(target) = literal.and_then(|raw| resolve_embed_target(source, &raw)) {
                let range = node.range();
                out.push((target, range.start, range.end));
            }
        }
        let before = stack.len();
        stack.extend(node.children());
        stack[before..].reverse();
    }
    out
}

/// Путь вставленного файла относительно корня vault; `None` — адрес не
/// локальный или уводит за корень.
fn resolve_embed_target(source: &Path, raw: &str) -> Option<PathBuf> {
    let raw = raw.trim().replace('\\', "/");
    if raw.is_empty() || raw.contains("://") {
        return None;
    }
    match raw.strip_prefix('/') {
        Some(rooted) => normalize(Path::new(rooted)),
        None => normalize(&source.parent().unwrap_or_else(|| Path::new("")).join(&raw)),
    }
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
    fn link_sites_ranges_are_byte_offsets_on_cyrillic() {
        let content = "см. [[Проект]] и [[б]]";
        let sites = link_sites(content);
        assert_eq!(sites.len(), 2);
        for site in &sites {
            assert!(content.is_char_boundary(site.byte_start));
            assert!(content.is_char_boundary(site.byte_end));
        }
        assert_eq!(
            &content[sites[0].byte_start..sites[0].byte_end],
            "[[Проект]]"
        );
        assert_eq!(&content[sites[1].byte_start..sites[1].byte_end], "[[б]]");
    }

    #[test]
    fn link_sites_finds_several_links_on_one_line() {
        let sites = link_sites("[[a]][[b]] [[ c ]]");
        let raws: Vec<&str> = sites.iter().map(|site| site.raw.as_str()).collect();
        assert_eq!(raws, vec!["a", "b", "c"]);
        // Диапазон включает пробелы внутри скобок, а `raw` их не содержит.
        assert_eq!(sites[2].byte_end - sites[2].byte_start, "[[ c ]]".len());
    }

    #[test]
    fn unclosed_and_empty_links_are_not_sites() {
        assert!(link_sites("текст [[без конца").is_empty());
        assert!(link_sites("пусто [[ ]] и [[]]").is_empty());
        // Незакрытая `[[` после закрытой ссылки не мешает первой.
        assert_eq!(link_sites("[[a]] потом [[b").len(), 1);
    }

    #[test]
    fn extract_links_is_the_raw_of_link_sites() {
        for content in [
            "",
            "[[a]] [[б]] [[ в ]]",
            "[[незакрытая",
            "[[ ]] [[x]]",
            "😀 [[эмодзи]] 😀",
        ] {
            let raws: Vec<String> = link_sites(content).into_iter().map(|s| s.raw).collect();
            assert_eq!(extract_links(content), raws, "{content}");
        }
    }

    #[test]
    fn board_extension_is_kept_and_other_dots_get_typ() {
        let source = Path::new("a/source.typ");
        assert_eq!(
            resolve_link_target(source, "схема.excalidraw"),
            Some(PathBuf::from("a/схема.excalidraw"))
        );
        assert_eq!(
            resolve_link_target(source, "глава.1"),
            Some(PathBuf::from("a/глава.1.typ"))
        );
    }

    #[test]
    fn link_text_table_round_trips_through_resolve() {
        let table: &[(&str, &str, &str)] = &[
            // сосед
            ("a/src.typ", "a/Проект.typ", "[[Проект]]"),
            // другая папка
            ("a/src.typ", "b/Проект.typ", "[[/b/Проект]]"),
            // из корня в папку и обратно
            ("src.typ", "docs/x.typ", "[[/docs/x]]"),
            ("docs/src.typ", "x.typ", "[[/x]]"),
            // корень
            ("src.typ", "x.typ", "[[x]]"),
            // доска сохраняет расширение
            ("a/src.typ", "a/схема.excalidraw", "[[схема.excalidraw]]"),
            ("a/src.typ", "b/схема.excalidraw", "[[/b/схема.excalidraw]]"),
            // точка в имени
            ("a/src.typ", "a/глава.1.typ", "[[глава.1]]"),
        ];
        for (source, target, expected) in table {
            let text = link_text(Path::new(source), Path::new(target));
            assert_eq!(text.as_deref(), Some(*expected), "{source} -> {target}");
            let inner = &text.unwrap()[2..expected.len() - 2];
            assert_eq!(
                resolve_link_target(Path::new(source), inner),
                Some(PathBuf::from(target))
            );
        }
    }

    #[test]
    fn link_text_refuses_what_cannot_be_written() {
        let source = Path::new("src.typ");
        assert_eq!(link_text(source, Path::new("a]]b.typ")), None);
        assert_eq!(link_text(source, Path::new("a/b]].typ")), None);
        assert_eq!(link_text(source, Path::new(" пробел.typ")), None);
        assert_eq!(link_text(source, Path::new("конец].typ")), None);
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

#[cfg(test)]
mod label_tests {
    use super::*;

    #[test]
    fn a_label_does_not_change_the_target_anywhere() {
        let content = "см. [[Проект|тот самый]] и [[/a/б | с пробелами ]] и [[в|]]";
        assert_eq!(extract_links(content), vec!["Проект", "/a/б", "в"]);
        let sites = link_sites(content);
        assert_eq!(sites[0].label.as_deref(), Some("тот самый"));
        assert_eq!(sites[1].label.as_deref(), Some("с пробелами"));
        assert_eq!(sites[2].label, None);
        assert_eq!(
            links_in(Path::new("src.typ"), content),
            vec![
                PathBuf::from("Проект.typ"),
                PathBuf::from("a/б.typ"),
                PathBuf::from("в.typ")
            ]
        );
    }

    #[test]
    fn an_empty_target_with_a_label_is_not_a_link() {
        assert!(link_sites("[[|только текст]]").is_empty());
    }
}
