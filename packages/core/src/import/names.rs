//! Имена назначения и разрешение ссылок по правилу Obsidian (фича 021,
//! research.md §6).
//!
//! Модуль чистый: на входе перечень файлов источника и перечень занятых имён
//! vault, на выходе план — что куда переносится и как по имени найти файл.
//! Пути везде строки с разделителем `/`: платформенных `PathBuf` здесь нет,
//! чтобы план не зависел от таргета (принцип II).

use std::collections::{HashMap, HashSet};

use super::{ImportJob, OnConflict, SkipReason};

/// Файл Markdown больше этого размера пропускается (plan.md → Scale/Scope).
pub const MAX_NOTE_BYTES: u64 = 5 * 1024 * 1024;

/// Форматы картинок, которые Typst собирает без доплат.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp"];

/// Суффикс «рядом» при совпадении имени (FR-009).
pub const BESIDE_SUFFIX: &str = "импорт";

/// Что за файл в источнике.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Note,
    Image,
    Other,
}

/// Расширение в нижнем регистре.
pub fn extension_of(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext.to_lowercase(),
        _ => String::new(),
    }
}

pub fn kind_of(path: &str) -> SourceKind {
    let ext = extension_of(path);
    if ext == "md" || ext == "markdown" {
        SourceKind::Note
    } else if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        SourceKind::Image
    } else {
        SourceKind::Other
    }
}

/// Папка пути (`""` — корень).
pub fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

pub fn file_name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Имя без расширения.
pub fn stem_of(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    }
}

/// Путь без последнего расширения.
fn without_extension(path: &str) -> &str {
    match path.rsplit_once('.') {
        Some((stem, ext)) if !ext.contains('/') && !stem.is_empty() && !stem.ends_with('/') => stem,
        _ => path,
    }
}

const FORBIDDEN: &[char] = &[':', '*', '?', '"', '<', '>', '|', '\\'];

/// Знаки, которые в записи `[[…]]` разметка Typst прочла бы не как текст.
const MARKUP_HOSTILE: &[char] = &['#', '$', '@', '`', '~', '_', '[', ']', '='];

/// Имя файла или папки, допустимое на любом устройстве: недопустимые знаки и
/// конечные точки/пробелы заменены на `-`, начальные пробелы убраны.
pub fn sanitize_component(name: &str) -> String {
    let trimmed = name.trim_start();
    let mut chars: Vec<char> = trimmed
        .chars()
        .map(|ch| {
            if FORBIDDEN.contains(&ch) || ch.is_control() {
                '-'
            } else {
                ch
            }
        })
        .collect();
    let mut end = chars.len();
    while end > 0 && (chars[end - 1] == '.' || chars[end - 1] == ' ') {
        end -= 1;
        chars[end] = '-';
    }
    let result: String = chars.into_iter().collect();
    if result.is_empty() || result == "." || result == ".." {
        "-".to_owned()
    } else {
        result
    }
}

/// Читается ли `[[текст]]` разметкой Typst как обычный текст — единственное
/// условие, при котором запись можно положить в заметку без экранирования
/// (экранирование сломало бы разбор целей ссылок).
pub fn is_link_safe(text: &str) -> bool {
    use typst::syntax::SyntaxKind;
    fn walk(node: &typst::syntax::SyntaxNode) -> bool {
        match node.kind() {
            SyntaxKind::Markup
            | SyntaxKind::Text
            | SyntaxKind::Space
            | SyntaxKind::LeftBracket
            | SyntaxKind::RightBracket => node.children().all(walk),
            _ => false,
        }
    }
    let root = typst::syntax::parse(&format!("[[{text}]]"));
    !root.diagnosis().errors && walk(&root)
}

/// Заменяет в имени всё, что мешает ссылке: используется, когда запись
/// `[[путь]]` небезопасна для разметки.
fn defuse(component: &str) -> String {
    component
        .chars()
        .map(|ch| {
            if MARKUP_HOSTILE.contains(&ch) {
                '-'
            } else {
                ch
            }
        })
        .collect()
}

/// Запись в плане для одного файла источника.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub source: String,
    pub destination: String,
    pub kind: SourceKind,
    pub size: u64,
    /// Имя назначения отличается от исходного не только расширением.
    pub renamed: Option<String>,
    /// Решение, принятое ещё при построении плана.
    pub skip: Option<SkipReason>,
    /// Перенесён прошлым запуском (журнал) — шаг его пропустит.
    pub done: bool,
}

/// План импорта (research.md §6, §7).
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// Шаги: заметки и прочие файлы, в порядке пути.
    pub items: Vec<PlannedFile>,
    /// Заметки, на которые можно ссылаться: индексы в `items`.
    by_stem: HashMap<String, Vec<usize>>,
    by_path: HashMap<String, usize>,
    /// Все файлы источника: нижний регистр имени → исходные пути.
    files_by_name: HashMap<String, Vec<String>>,
    files_by_path: HashMap<String, String>,
    /// Занятые имена назначения (нижний регистр) — планом и vault.
    pub taken: HashSet<String>,
    /// Что лежало в vault до импорта (нижний регистр).
    existing: HashSet<String>,
    /// Корень назначения без краевых `/`.
    pub root: String,
    pub notes: u32,
    pub images: u32,
    pub others: u32,
}

/// Результат поиска заметки по ссылке.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// Единственное совпадение.
    One {
        destination: String,
    },
    /// Совпадений несколько, выбрано ближайшее (FR-005, edge cases).
    Ambiguous {
        destination: String,
    },
    None,
}

fn lower(path: &str) -> String {
    path.to_lowercase()
}

fn join(root: &str, relative: &str) -> String {
    if root.is_empty() {
        relative.to_owned()
    } else {
        format!("{root}/{relative}")
    }
}

/// Расстояние между папками по дереву: шагов вверх и вниз.
fn tree_distance(from_dir: &str, to_dir: &str) -> usize {
    let from: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
    let to: Vec<&str> = to_dir.split('/').filter(|s| !s.is_empty()).collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    (from.len() - common) + (to.len() - common)
}

/// Ближайший к `from_dir` из кандидатов; при равенстве — короче путь, затем
/// по алфавиту (результат не зависит от порядка обхода).
fn nearest<'a>(from_dir: &str, candidates: &[&'a str]) -> &'a str {
    candidates
        .iter()
        .min_by_key(|candidate| {
            (
                tree_distance(from_dir, dir_of(candidate)),
                candidate.len(),
                **candidate,
            )
        })
        .copied()
        .unwrap_or("")
}

impl Plan {
    /// Строит план. `existing` — занятые пути vault в нижнем регистре через
    /// `/`; `done` — исходный путь → назначение из журнала.
    pub fn build(
        files: &[(String, u64)],
        job: &ImportJob,
        existing: &HashSet<String>,
        done: &HashMap<String, String>,
    ) -> Plan {
        let root = job.destination.trim_matches('/').to_owned();
        let mut plan = Plan {
            root,
            taken: existing.clone(),
            existing: existing.clone(),
            ..Plan::default()
        };

        let mut sorted: Vec<&(String, u64)> = files.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        for (source, _) in &sorted {
            plan.files_by_name
                .entry(lower(file_name_of(source)))
                .or_default()
                .push(source.clone());
            plan.files_by_path.insert(lower(source), source.clone());
            match kind_of(source) {
                SourceKind::Note => plan.notes += 1,
                SourceKind::Image => plan.images += 1,
                SourceKind::Other => plan.others += 1,
            }
            // Уже перенесённое занимает свои имена первым: ссылки между старым
            // и новым сходятся (FR-011).
            if let Some(destination) = done.get(source) {
                plan.taken.insert(lower(destination));
            }
        }

        // Шаги: сначала заметки, потом прочее. Картинка, на которую сослалась
        // заметка, к своей очереди уже скопирована и шаг её не повторит.
        for notes_pass in [true, false] {
            for (source, size) in &sorted {
                let kind = kind_of(source);
                if (kind == SourceKind::Note) != notes_pass {
                    continue;
                }
                let item = match done.get(source) {
                    Some(destination) => PlannedFile {
                        source: source.clone(),
                        destination: destination.clone(),
                        kind,
                        size: *size,
                        renamed: None,
                        skip: None,
                        done: true,
                    },
                    None if notes_pass => plan.plan_note(source, *size, job),
                    None => plan.plan_other(source, *size, kind, job),
                };
                plan.push_item(item);
            }
        }
        plan
    }

    fn push_item(&mut self, item: PlannedFile) {
        let index = self.items.len();
        // Ссылаться можно только на заметку, которая окажется в vault.
        if item.kind == SourceKind::Note
            && !(item.skip.is_some() && item.skip != Some(SkipReason::ExistsSkipped))
        {
            let source = item.source.clone();
            self.by_stem
                .entry(lower(stem_of(file_name_of(&source))))
                .or_default()
                .push(index);
            self.by_path
                .insert(lower(without_extension(&source)), index);
        }
        self.items.push(item);
    }

    /// Имя заметки в назначении: `.md` → `.typ`, недопустимое заменено,
    /// совпадения внутри плана и с vault разведены.
    fn plan_note(&mut self, source: &str, size: u64, job: &ImportJob) -> PlannedFile {
        let (dir_parts, original_stem) = split_source(source);
        let mut dirs: Vec<String> = dir_parts.iter().map(|d| sanitize_component(d)).collect();
        let mut stem = sanitize_component(&original_stem);

        // Запись `[[путь]]` должна читаться разметкой как текст.
        let probe = |dirs: &[String], stem: &str| -> String {
            let mut parts = dirs.to_vec();
            parts.push(stem.to_owned());
            parts.join("/")
        };
        if !is_link_safe(&probe(&dirs, &stem)) {
            dirs = dirs.iter().map(|d| defuse(d)).collect();
            stem = defuse(&stem);
        }
        let mut renamed = (dirs != dir_parts || stem != original_stem).then(|| {
            let mut parts = dirs.clone();
            parts.push(format!("{stem}.typ"));
            parts.join("/")
        });

        let relative_dir = dirs.join("/");
        let build = |stem: &str| -> String {
            let name = format!("{stem}.typ");
            let relative = if relative_dir.is_empty() {
                name
            } else {
                format!("{relative_dir}/{name}")
            };
            join(&self.root, &relative)
        };

        let mut skip = None;
        let mut candidate = build(&stem);
        if size > MAX_NOTE_BYTES {
            skip = Some(SkipReason::TooLarge);
        } else if self.existing.contains(&lower(&candidate)) && job.on_conflict == OnConflict::Skip
        {
            skip = Some(SkipReason::ExistsSkipped);
        } else if self.taken.contains(&lower(&candidate)) {
            let beside = self.existing.contains(&lower(&candidate));
            let base = stem.clone();
            let mut n = 1;
            loop {
                n += 1;
                let suffixed = match (beside, n) {
                    (true, 2) => format!("{base} ({BESIDE_SUFFIX})"),
                    (true, _) => format!("{base} ({BESIDE_SUFFIX} {})", n - 1),
                    (false, _) => format!("{base} ({n})"),
                };
                candidate = build(&suffixed);
                if !self.taken.contains(&lower(&candidate)) {
                    break;
                }
            }
            renamed = Some(candidate.clone());
        }
        if skip.is_none() {
            self.taken.insert(lower(&candidate));
        }
        PlannedFile {
            source: source.to_owned(),
            destination: candidate,
            kind: SourceKind::Note,
            size,
            renamed,
            skip,
            done: false,
        }
    }

    fn plan_other(
        &mut self,
        source: &str,
        size: u64,
        kind: SourceKind,
        job: &ImportJob,
    ) -> PlannedFile {
        let parts: Vec<&str> = source.split('/').collect();
        let sanitized: Vec<String> = parts.iter().map(|p| sanitize_component(p)).collect();
        let renamed_at_all = sanitized
            .iter()
            .map(String::as_str)
            .ne(parts.iter().copied());
        let relative = sanitized.join("/");
        let candidate = join(&self.root, &relative);
        let skip = if !job.copy_other_files {
            Some(SkipReason::OtherFile)
        } else {
            None
        };
        PlannedFile {
            source: source.to_owned(),
            destination: candidate,
            kind,
            size,
            renamed: renamed_at_all.then_some(relative),
            skip,
            done: false,
        }
    }

    /// Поиск заметки по цели ссылки (research.md §6): точный путь → единственная
    /// заметка с таким именем → ближайшая по дереву (с пометкой неоднозначности).
    pub fn resolve_note(&self, from_source: &str, target: &str) -> Found {
        let target = target.trim().replace('\\', "/");
        let target = target.trim_start_matches('/');
        let target = match target.strip_suffix(".md") {
            Some(rest) => rest,
            None => target.strip_suffix(".markdown").unwrap_or(target),
        };
        if target.is_empty() {
            return Found::None;
        }
        let from_dir = dir_of(from_source);

        if target.contains('/') {
            // Относительно папки заметки, затем от корня, затем по хвосту пути.
            let relative = normalize_join(from_dir, target);
            for candidate in [relative.as_str(), target] {
                if let Some(&index) = self.by_path.get(&lower(candidate)) {
                    return Found::One {
                        destination: self.items[index].destination.clone(),
                    };
                }
            }
            let tail = format!("/{}", lower(target));
            let hits: Vec<usize> = self
                .by_path
                .iter()
                .filter(|(path, _)| format!("/{path}").ends_with(&tail))
                .map(|(_, index)| *index)
                .collect();
            return self.pick(from_dir, &hits);
        }

        match self.by_stem.get(&lower(target)) {
            Some(hits) => self.pick(from_dir, hits),
            None => Found::None,
        }
    }

    fn pick(&self, from_dir: &str, hits: &[usize]) -> Found {
        match hits {
            [] => Found::None,
            [one] => Found::One {
                destination: self.items[*one].destination.clone(),
            },
            many => {
                let sources: Vec<&str> = many
                    .iter()
                    .map(|index| self.items[*index].source.as_str())
                    .collect();
                let best = nearest(from_dir, &sources);
                let index = many
                    .iter()
                    .find(|index| self.items[**index].source == best)
                    .copied()
                    .unwrap_or(many[0]);
                Found::Ambiguous {
                    destination: self.items[index].destination.clone(),
                }
            }
        }
    }

    /// Файл источника (картинка, вложение) по цели: путь от папки заметки, от
    /// корня, затем по имени — ближайший.
    pub fn resolve_file(&self, from_source: &str, target: &str) -> Option<String> {
        let target = target.trim().replace('\\', "/");
        let target = target.trim_start_matches('/');
        if target.is_empty() {
            return None;
        }
        let from_dir = dir_of(from_source);
        if target.contains('/') {
            let relative = normalize_join(from_dir, target);
            for candidate in [relative.as_str(), target] {
                if let Some(path) = self.files_by_path.get(&lower(candidate)) {
                    return Some(path.clone());
                }
            }
        }
        let name = lower(file_name_of(target));
        let hits = self.files_by_name.get(&name)?;
        let refs: Vec<&str> = hits.iter().map(String::as_str).collect();
        // Путь в ссылке сужает выбор: берём только те, что кончаются им.
        let suffix = format!("/{}", lower(target));
        let narrowed: Vec<&str> = refs
            .iter()
            .copied()
            .filter(|path| target.contains('/') && format!("/{}", lower(path)).ends_with(&suffix))
            .collect();
        let pool = if narrowed.is_empty() { refs } else { narrowed };
        Some(nearest(from_dir, &pool).to_owned())
    }

    /// Назначение исходного файла, если он в плане.
    pub fn destination_of(&self, source: &str) -> Option<&str> {
        self.items
            .iter()
            .find(|item| item.source == source)
            .map(|item| item.destination.as_str())
    }
}

/// `папка/относительный/путь` с разбором `.` и `..`.
fn normalize_join(dir: &str, relative: &str) -> String {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// Исходный путь заметки → (папки, имя без расширения).
fn split_source(source: &str) -> (Vec<String>, String) {
    let mut parts: Vec<String> = source.split('/').map(str::to_owned).collect();
    let name = parts.pop().unwrap_or_default();
    let stem = stem_of(&name).to_owned();
    (parts, stem)
}

/// Идентификатор журнала: FNV-1a от источника и назначения, 16 hex-знаков.
pub fn journal_id(source_hint: &str, destination: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in source_hint
        .bytes()
        .chain(std::iter::once(0))
        .chain(destination.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(on_conflict: OnConflict) -> ImportJob {
        ImportJob {
            destination: "Импорт".into(),
            on_conflict,
            copy_other_files: false,
            ..ImportJob::default()
        }
    }

    fn files(paths: &[&str]) -> Vec<(String, u64)> {
        paths.iter().map(|p| ((*p).to_owned(), 10)).collect()
    }

    fn plan(paths: &[&str], existing: &[&str], on_conflict: OnConflict) -> Plan {
        let existing: HashSet<String> = existing.iter().map(|p| p.to_lowercase()).collect();
        Plan::build(&files(paths), &job(on_conflict), &existing, &HashMap::new())
    }

    #[test]
    fn invalid_characters_and_trailing_dots_become_dashes() {
        assert_eq!(sanitize_component("Что? Как: так"), "Что- Как- так");
        assert_eq!(sanitize_component("конец. "), "конец--");
        assert_eq!(sanitize_component("  начало"), "начало");
        assert_eq!(sanitize_component("???"), "---");
        assert_eq!(sanitize_component(".."), "--");
    }

    #[test]
    fn the_tree_is_kept_and_the_extension_changes() {
        let plan = plan(&["a/b/Заметка.md", "Корень.md"], &[], OnConflict::Skip);
        let dests: Vec<&str> = plan.items.iter().map(|i| i.destination.as_str()).collect();
        assert_eq!(dests, ["Импорт/a/b/Заметка.typ", "Импорт/Корень.typ"]);
    }

    #[test]
    fn names_colliding_after_replacement_get_a_counter() {
        let plan = plan(&["a?.md", "a-.md", "a:.md"], &[], OnConflict::Skip);
        let dests: Vec<&str> = plan.items.iter().map(|i| i.destination.as_str()).collect();
        assert_eq!(
            dests,
            ["Импорт/a-.typ", "Импорт/a- (2).typ", "Импорт/a- (3).typ"]
        );
        assert!(plan.items[1].renamed.is_some());
    }

    #[test]
    fn an_existing_destination_is_skipped_or_goes_beside() {
        let skipped = plan(&["N.md"], &["Импорт/N.typ"], OnConflict::Skip);
        assert_eq!(skipped.items[0].skip, Some(SkipReason::ExistsSkipped));
        let beside = plan(&["N.md"], &["Импорт/N.typ"], OnConflict::Beside);
        assert_eq!(beside.items[0].destination, "Импорт/N (импорт).typ");
        assert!(beside.items[0].skip.is_none());
    }

    #[test]
    fn a_name_resolves_to_the_nearest_and_reports_ambiguity() {
        let plan = plan(
            &["a/x/Идея.md", "b/Идея.md", "a/Текст.md", "b/Другая.md"],
            &[],
            OnConflict::Skip,
        );
        assert_eq!(
            plan.resolve_note("a/Текст.md", "Идея"),
            Found::Ambiguous {
                destination: "Импорт/a/x/Идея.typ".into()
            }
        );
        assert_eq!(
            plan.resolve_note("b/Другая.md", "идея"),
            Found::Ambiguous {
                destination: "Импорт/b/Идея.typ".into()
            }
        );
        assert_eq!(
            plan.resolve_note("b/Другая.md", "Текст"),
            Found::One {
                destination: "Импорт/a/Текст.typ".into()
            }
        );
        assert_eq!(plan.resolve_note("b/Другая.md", "Нет такой"), Found::None);
    }

    #[test]
    fn a_path_in_the_link_wins_over_the_name() {
        let plan = plan(&["a/Идея.md", "b/Идея.md"], &[], OnConflict::Skip);
        assert_eq!(
            plan.resolve_note("a/Идея.md", "b/Идея"),
            Found::One {
                destination: "Импорт/b/Идея.typ".into()
            }
        );
        assert_eq!(
            plan.resolve_note("a/Идея.md", "b/Идея.md#Раздел"),
            Found::None,
            "якорь отрезает вызывающий, не разрешение"
        );
    }

    #[test]
    fn files_resolve_by_relative_path_then_by_name() {
        let plan = plan(
            &["n/Заметка.md", "n/img/a.png", "other/a.png"],
            &[],
            OnConflict::Skip,
        );
        assert_eq!(
            plan.resolve_file("n/Заметка.md", "img/a.png").as_deref(),
            Some("n/img/a.png")
        );
        assert_eq!(
            plan.resolve_file("n/Заметка.md", "a.png").as_deref(),
            Some("n/img/a.png")
        );
        assert_eq!(plan.resolve_file("n/Заметка.md", "нет.png"), None);
    }

    #[test]
    fn markup_hostile_names_are_defused_so_that_links_stay_text() {
        let plan = plan(&["C# заметки.md", "_скрытая.md"], &[], OnConflict::Skip);
        for item in &plan.items {
            assert!(
                is_link_safe(item.destination.trim_end_matches(".typ")),
                "{}",
                item.destination
            );
            assert!(item.renamed.is_some());
        }
    }

    #[test]
    fn journal_id_depends_on_both_sides() {
        let a = journal_id("vault", "Импорт");
        assert_eq!(a.len(), 16);
        assert_ne!(a, journal_id("vault", "Другое"));
        assert_ne!(a, journal_id("другой", "Импорт"));
        assert_eq!(a, journal_id("vault", "Импорт"));
    }
}

// ── Разрешение ссылок и вложений для перевода ────────────────────────────────

/// Что нашлось по цели ссылки на заметку.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedLink {
    /// Заметка есть; `inner` — запись внутри `[[…]]`, которая из папки этой
    /// заметки разрешается в импортированную.
    Note { inner: String, ambiguous: bool },
    /// Цель — файл, но не заметка (вложение): ссылаться на него нечем.
    File,
    /// Цели нет.
    Missing,
}

/// Что нашлось по цели картинки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedImage {
    /// Путь для `#image(…)` относительно папки заметки.
    Image {
        path: String,
    },
    Missing,
}

/// Всё, что переводу нужно знать о мире вокруг заметки.
pub trait Resolver {
    fn link(&mut self, target: &str) -> ResolvedLink;
    fn image(&mut self, target: &str) -> ResolvedImage;
}

/// Вложения, которые пакет разложил или собирается разложить (research.md §4).
#[derive(Debug, Clone, Default)]
pub struct Attachments {
    /// Источник → назначение; один файл копируется один раз — по первой
    /// заметке, которая на него сослалась.
    pub claimed: std::collections::HashMap<String, String>,
    /// Занятые имена назначения (нижний регистр).
    pub taken: HashSet<String>,
    /// Ещё не скопированные: (источник, назначение).
    pub pending: Vec<(String, String)>,
}

impl Attachments {
    pub fn new(taken: HashSet<String>) -> Self {
        Attachments {
            taken,
            ..Attachments::default()
        }
    }

    /// Назначение вложения; первое обращение резервирует свободное имя.
    pub fn claim(
        &mut self,
        source: &str,
        note_destination: &str,
        placement: crate::boards::placement::AssetsPlacement,
        sections: &[crate::vault::sections::SectionMark],
    ) -> String {
        if let Some(destination) = self.claimed.get(source) {
            return destination.clone();
        }
        let folder = crate::boards::placement::note_assets_folder(
            placement,
            std::path::Path::new(note_destination),
            sections,
        )
        .map(|folder| folder.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| dir_of(note_destination).to_owned());
        let name = sanitize_component(file_name_of(source));
        let (stem, extension) = match name.rsplit_once('.') {
            Some((stem, extension)) if !stem.is_empty() => {
                (stem.to_owned(), format!(".{extension}"))
            }
            _ => (name.clone(), String::new()),
        };
        let build = |stem: &str| {
            if folder.is_empty() {
                format!("{stem}{extension}")
            } else {
                format!("{folder}/{stem}{extension}")
            }
        };
        let mut candidate = build(&stem);
        let mut n = 1;
        while self.taken.contains(&lower(&candidate)) {
            n += 1;
            candidate = build(&format!("{stem} ({n})"));
        }
        self.taken.insert(lower(&candidate));
        self.claimed.insert(source.to_owned(), candidate.clone());
        self.pending.push((source.to_owned(), candidate.clone()));
        candidate
    }
}

/// [`Resolver`] пакета: план, вложения и положение заметки.
pub struct NoteResolver<'a> {
    pub plan: &'a Plan,
    pub attachments: &'a mut Attachments,
    /// Путь заметки в источнике и в vault.
    pub source: &'a str,
    pub destination: &'a str,
    pub placement: crate::boards::placement::AssetsPlacement,
    pub sections: &'a [crate::vault::sections::SectionMark],
    /// Вложения, которые прочитать или записать не удалось: картинка на них
    /// считается ненайденной, чтобы заметка не ссылалась на несуществующее.
    pub unavailable: &'a HashSet<String>,
}

impl Resolver for NoteResolver<'_> {
    fn link(&mut self, target: &str) -> ResolvedLink {
        let found = self.plan.resolve_note(self.source, target);
        let (destination, ambiguous) = match found {
            Found::One { destination } => (destination, false),
            Found::Ambiguous { destination } => (destination, true),
            Found::None => {
                return if self.plan.resolve_file(self.source, target).is_some() {
                    ResolvedLink::File
                } else {
                    ResolvedLink::Missing
                };
            }
        };
        let text = crate::graph::link_text(
            std::path::Path::new(self.destination),
            std::path::Path::new(&destination),
        );
        match text {
            Some(text) => ResolvedLink::Note {
                inner: text[2..text.len() - 2].to_owned(),
                ambiguous,
            },
            None => ResolvedLink::Missing,
        }
    }

    fn image(&mut self, target: &str) -> ResolvedImage {
        let Some(source) = self.plan.resolve_file(self.source, target) else {
            return ResolvedImage::Missing;
        };
        if kind_of(&source) != SourceKind::Image || self.unavailable.contains(&source) {
            return ResolvedImage::Missing;
        }
        let destination =
            self.attachments
                .claim(&source, self.destination, self.placement, self.sections);
        let from = std::path::Path::new(self.destination)
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_path_buf();
        ResolvedImage::Image {
            path: crate::boards::placement::relative_to(&from, std::path::Path::new(&destination)),
        }
    }
}

/// Мир одной заметки для разового перевода (тесты, предпросмотр): список
/// соседних файлов источника и положение заметки.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ConvertContext {
    /// Путь заметки в источнике.
    pub source: String,
    /// Папка назначения в vault.
    #[serde(default)]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub destination: String,
    /// Пути файлов источника (заметки и вложения), которые ссылки могут найти.
    #[serde(default)]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub files: Vec<String>,
}

/// [`Resolver`] с собственным планом — для [`ConvertContext`].
pub struct StandaloneResolver {
    plan: Plan,
    attachments: Attachments,
    source: String,
    destination: String,
    unavailable: HashSet<String>,
}

impl ConvertContext {
    pub fn resolver(&self) -> StandaloneResolver {
        let job = ImportJob {
            destination: self.destination.clone(),
            copy_other_files: true,
            ..ImportJob::default()
        };
        let mut files: Vec<(String, u64)> =
            self.files.iter().map(|path| (path.clone(), 1)).collect();
        if !files.iter().any(|(path, _)| *path == self.source) {
            files.push((self.source.clone(), 1));
        }
        let plan = Plan::build(&files, &job, &HashSet::new(), &HashMap::new());
        let destination = plan
            .destination_of(&self.source)
            .unwrap_or_default()
            .to_owned();
        StandaloneResolver {
            attachments: Attachments::new(plan.taken.clone()),
            plan,
            source: self.source.clone(),
            destination,
            unavailable: HashSet::new(),
        }
    }
}

impl Resolver for StandaloneResolver {
    fn link(&mut self, target: &str) -> ResolvedLink {
        self.note_resolver().link(target)
    }

    fn image(&mut self, target: &str) -> ResolvedImage {
        self.note_resolver().image(target)
    }
}

impl StandaloneResolver {
    fn note_resolver(&mut self) -> NoteResolver<'_> {
        NoteResolver {
            plan: &self.plan,
            attachments: &mut self.attachments,
            source: &self.source,
            destination: &self.destination,
            placement: Default::default(),
            sections: &[],
            unavailable: &self.unavailable,
        }
    }
}
