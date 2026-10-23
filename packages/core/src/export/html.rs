//! Экспорт в HTML (фича 034).
//!
//! Сборка — тот же цикл, что у предпросмотра и PDF, последний шаг —
//! `typst-html` (research.md §1). Элементы, которые HTML-вывод пропускает,
//! выводятся рисунком через `html.frame` и называются в итоге (§2). Ссылки
//! `[[…]]` на заметки набора становятся относительными ссылками на страницы,
//! вне набора — помеченным текстом (§3). Набор из нескольких заметок получает
//! оглавление `index.html`; на web страницы собираются в zip (§4).

use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::folder::export_folder;
use super::{ExportOutcome, ExportReport, ExportWriteProblem};
use crate::graph::{link_sites, resolve_link_target};
use crate::messages::AppMessage;
use crate::preview::compile::{build_with_templates, InlineExecutor, Render};
use crate::preview::snapshot::SnapshotError;
use crate::preview::{Diagnostic, DiagnosticSeverity, FontSet, PackageCache};
use crate::vault::backend::{ReadError, VaultBackend, WriteError};
use crate::vault::vault_settings::VaultOverrides;
use crate::vault::FolderAccess;

/// Подписи навигации — из переводов интерфейса.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SiteLabels {
    /// «Оглавление».
    pub contents: String,
    /// Подсказка у ссылки на заметку вне экспорта: «вне экспорта».
    pub outside: String,
}

/// Сколько раз пересобирать с рисунками пропущенных элементов.
const FRAME_ROUNDS: usize = 2;
const IGNORED: &str = " was ignored during HTML export";
const UNSTABLE: &str = "html export is under active development";

/// Путь страницы заметки в папке экспорта: структура папок сохраняется.
pub fn html_path(note_path: &Path, folder: &str) -> PathBuf {
    let mut path = PathBuf::from(folder);
    for component in note_path.components() {
        if let Component::Normal(part) = component {
            path.push(part);
        }
    }
    path.set_extension("html");
    path
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Относительный адрес от страницы `from` к странице `to` (оба — пути в vault).
fn relative(from: &Path, to: &Path) -> String {
    let from_dir: Vec<_> = from
        .parent()
        .map(|parent| parent.components().collect())
        .unwrap_or_default();
    let to_parts: Vec<_> = to.components().collect();
    let common = from_dir
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<String> =
        std::iter::repeat_n("..".to_owned(), from_dir.len() - common).collect();
    parts.extend(
        to_parts[common..]
            .iter()
            .map(|part| part.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

/// Строка Typst в кавычках.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Подпись ссылки как содержимое Typst: знаки разметки экранируются.
fn escaped_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(
            ch,
            '[' | ']' | '#' | '*' | '_' | '`' | '$' | '\\' | '<' | '>' | '@' | '='
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// `[[цель|подпись]]` → ссылка на страницу набора или помеченный текст (§3).
fn rewrite_links(
    text: &str,
    note_path: &str,
    folder: &str,
    site: &HashSet<String>,
    labels: &SiteLabels,
) -> String {
    let source = Path::new(note_path);
    let page = html_path(source, folder);
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for site_link in link_sites(text) {
        // Встраивания (031) — не ссылки на странице: их показывает сборка.
        if site_link.call.is_some() {
            continue;
        }
        out.push_str(&text[cursor..site_link.byte_start]);
        let target = resolve_link_target(source, &site_link.raw);
        let shown = site_link.label.clone().unwrap_or_else(|| {
            crate::links::title_of(&site_link.raw.replace('\\', "/"))
                .trim()
                .to_owned()
        });
        let shown = if shown.is_empty() {
            site_link.raw.clone()
        } else {
            shown
        };
        match target.filter(|target| site.contains(&slashed(target))) {
            Some(target) => {
                let href = relative(&page, &html_path(&target, folder));
                out.push_str(&format!(
                    "#link({})[{}]",
                    quoted(&href),
                    escaped_markup(&shown)
                ));
            }
            None => out.push_str(&format!(
                "#html.span(class: \"ts-outside\", title: {})[{}]",
                quoted(&labels.outside),
                escaped_markup(&shown)
            )),
        }
        cursor = site_link.byte_end;
    }
    out.push_str(&text[cursor..]);
    out
}

/// Имена элементов, пропущенных HTML-выводом, из предупреждений сборки.
fn ignored_elements(diagnostics: &[Diagnostic]) -> Vec<String> {
    let mut out: Vec<String> = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.message.strip_suffix(IGNORED))
        .map(|name| name.trim().trim_matches('`').to_owned())
        .filter(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_')
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Правило «вывести рисунком» одной строкой без перевода — номера строк
/// диагностик не сдвигаются (research.md §2).
fn frame_rule(names: &[String]) -> String {
    let selectors: Vec<String> = names
        .iter()
        .map(|name| format!("selector({name})"))
        .collect();
    let selector = selectors
        .split_first()
        .map(|(first, rest)| {
            rest.iter()
                .fold(first.clone(), |acc, next| format!("{acc}.or({next})"))
        })
        .unwrap_or_default();
    format!("#show {selector}: html.frame;")
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Стиль помеченных ссылок и навигации — в странице, без внешних файлов.
const SITE_STYLE: &str = "<style>.ts-outside{color:#666;border-bottom:1px dotted #999}\
.ts-site-nav{font:14px system-ui,sans-serif;margin:0 0 1em}</style>";

/// Навигация и стиль в готовую страницу.
fn decorate(html: String, page: &Path, folder: &str, multi: bool, labels: &SiteLabels) -> String {
    let mut html = match html.find("</head>") {
        Some(at) => format!("{}{SITE_STYLE}{}", &html[..at], &html[at..]),
        None => html,
    };
    if multi {
        let index = relative(page, &PathBuf::from(folder).join("index.html"));
        let nav = format!(
            "<nav class=\"ts-site-nav\"><a href=\"{}\">← {}</a></nav>",
            escape_html(&index),
            escape_html(&labels.contents)
        );
        if let Some(at) = html.find("<body>") {
            let at = at + "<body>".len();
            html = format!("{}{nav}{}", &html[..at], &html[at..]);
        }
    }
    html
}

async fn write_file<B: VaultBackend>(
    backend: &mut B,
    target: &Path,
    bytes: &[u8],
) -> Result<(), ExportWriteProblem> {
    if matches!(
        backend.folder_access(target).await,
        FolderAccess::Present | FolderAccess::NotWritable { .. }
    ) {
        return Err(ExportWriteProblem::TargetIsFolder);
    }
    let written = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => backend.ensure_folder(parent).await,
        _ => Ok(()),
    };
    match written.and(Ok(())) {
        Ok(()) => backend.write_bytes(target, bytes).await,
        Err(error) => Err(error),
    }
    .map_err(|error| match error {
        WriteError::AccessDenied => ExportWriteProblem::AccessDenied,
        _ => ExportWriteProblem::Io,
    })
}

/// Экспортирует заметку страницей HTML (FR-001). `site` — пути всех заметок
/// экспорта (сама заметка входит): на них ссылки ведут на страницы (FR-002).
#[allow(clippy::too_many_arguments)]
pub async fn export_note_html<B: VaultBackend>(
    backend: &mut B,
    packages: &PackageCache,
    note_path: &str,
    source_text: Option<&str>,
    fonts: FontSet,
    overrides: &VaultOverrides,
    site: &[String],
    labels: &SiteLabels,
) -> ExportReport {
    let folder = export_folder(overrides);
    let target = html_path(Path::new(note_path), &folder);
    let report = |outcome, diagnostics, as_images: Vec<String>| ExportReport {
        note_path: note_path.to_owned(),
        pdf_path: slashed(&target),
        outcome,
        diagnostics,
        as_images,
    };

    let text = match source_text {
        Some(text) => text.to_owned(),
        None => match backend.read(Path::new(note_path)).await {
            Ok(text) => text,
            Err(ReadError::NotFound) => {
                return report(ExportOutcome::NoteMissing, Vec::new(), Vec::new())
            }
            Err(ReadError::Io | ReadError::TooLarge) => {
                return report(ExportOutcome::NoteUnreadable, Vec::new(), Vec::new())
            }
        },
    };
    let set: HashSet<String> = site.iter().cloned().collect();
    let rewritten = rewrite_links(&text, note_path, &folder, &set, labels);

    let mut framed: Vec<String> = Vec::new();
    let mut built = None;
    for _ in 0..=FRAME_ROUNDS {
        let source = if framed.is_empty() {
            rewritten.clone()
        } else {
            format!("{}{rewritten}", frame_rule(&framed))
        };
        let result = build_with_templates(
            &InlineExecutor,
            backend,
            packages,
            note_path,
            &source,
            0,
            fonts.clone(),
            &overrides.template_policy(),
            &Render::Html,
        )
        .await;
        let (document, html) = match result {
            Ok(result) => result,
            Err(SnapshotError::OutsideVault { path }) => {
                return report(
                    ExportOutcome::CompileFailed,
                    vec![Diagnostic::app(
                        DiagnosticSeverity::Error,
                        AppMessage::NoteOutsideVault { path },
                    )],
                    Vec::new(),
                )
            }
        };
        let missing: Vec<String> = ignored_elements(&document.diagnostics)
            .into_iter()
            .filter(|name| !framed.contains(name))
            .collect();
        let done = missing.is_empty() || html.is_none();
        framed.extend(missing);
        framed.sort();
        built = Some((document, html));
        if done {
            break;
        }
    }
    let Some((document, html)) = built else {
        return report(ExportOutcome::CompileFailed, Vec::new(), framed);
    };
    // Служебные предупреждения HTML-вывода пользователю не показываются: о
    // пропущенном — итог `as_images`.
    let diagnostics: Vec<Diagnostic> = document
        .diagnostics
        .into_iter()
        .filter(|d| !d.message.starts_with(UNSTABLE) && !d.message.ends_with(IGNORED))
        .collect();
    let Some(bytes) = html else {
        return report(ExportOutcome::CompileFailed, diagnostics, framed);
    };
    let page = decorate(
        String::from_utf8_lossy(&bytes).into_owned(),
        &html_path(Path::new(note_path), ""),
        "",
        site.len() > 1,
        labels,
    );
    match write_file(backend, &target, page.as_bytes()).await {
        Ok(()) => report(
            ExportOutcome::Written {
                bytes: page.len() as u64,
            },
            diagnostics
                .into_iter()
                .filter(|d| d.severity == DiagnosticSeverity::Warning)
                .collect(),
            framed,
        ),
        Err(reason) => report(ExportOutcome::WriteFailed { reason }, diagnostics, framed),
    }
}

/// Оглавление сайта `index.html` (FR-002): страницы по папкам. Ответ — путь
/// оглавления в vault.
pub async fn export_site_index<B: VaultBackend>(
    backend: &mut B,
    overrides: &VaultOverrides,
    site: &[String],
    labels: &SiteLabels,
) -> Result<String, ExportWriteProblem> {
    let folder = export_folder(overrides);
    let mut by_folder: BTreeMap<String, Vec<&String>> = BTreeMap::new();
    for path in site {
        let parent = path
            .rsplit_once('/')
            .map_or(String::new(), |(parent, _)| parent.to_owned());
        by_folder.entry(parent).or_default().push(path);
    }
    let mut body = format!("<h1>{}</h1>", escape_html(&labels.contents));
    for (parent, paths) in by_folder {
        if !parent.is_empty() {
            body.push_str(&format!("<h2>{}</h2>", escape_html(&parent)));
        }
        body.push_str("<ul>");
        for path in paths {
            let href = relative(Path::new("index.html"), &html_path(Path::new(path), ""));
            body.push_str(&format!(
                "<li><a href=\"{}\">{}</a></li>",
                escape_html(&href),
                escape_html(&crate::links::title_of(path))
            ));
        }
        body.push_str("</ul>");
    }
    let page = format!(
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n<style>body{{font:16px/1.5 system-ui,sans-serif;max-width:46em;margin:2em auto;padding:0 1em}}</style>\n\
         </head>\n<body>{body}</body>\n</html>\n",
        escape_html(&labels.contents)
    );
    let target = PathBuf::from(&folder).join("index.html");
    write_file(backend, &target, page.as_bytes()).await?;
    Ok(slashed(&target))
}

// ── Архив ───────────────────────────────────────────────────────────────────

/// Zip без сжатия: локальные заголовки, центральный каталог, CRC32.
fn zip_store(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let mut crc = flate2::Crc::new();
        crc.update(data);
        let crc = crc.sum();
        let offset = out.len() as u32;
        let name_bytes = name.as_bytes();
        let size = data.len() as u32;
        // Локальный заголовок; флаг 0x0800 — имя в UTF-8.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0x0800u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);
        // Запись центрального каталога.
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let central_offset = out.len() as u32;
    let central_size = central.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Архив записанных страниц и оглавления для скачивания на web (FR-005).
/// Имена в архиве — относительно папки экспорта; ненайденные страницы
/// пропускаются.
pub async fn export_site_archive<B: VaultBackend>(
    backend: &B,
    overrides: &VaultOverrides,
    site: &[String],
) -> Vec<u8> {
    let folder = export_folder(overrides);
    let mut files = Vec::new();
    let index = PathBuf::from(&folder).join("index.html");
    if let Ok(text) = backend.read(&index).await {
        files.push(("index.html".to_owned(), text.into_bytes()));
    }
    for path in site {
        let page = html_path(Path::new(path), &folder);
        if let Ok(text) = backend.read(&page).await {
            files.push((slashed(&html_path(Path::new(path), "")), text.into_bytes()));
        }
    }
    zip_store(&files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_addresses_between_pages() {
        assert_eq!(
            relative(Path::new("a/b/x.html"), Path::new("a/c/y.html")),
            "../c/y.html"
        );
        assert_eq!(
            relative(Path::new("x.html"), Path::new("a/y.html")),
            "a/y.html"
        );
        assert_eq!(
            relative(Path::new("a/x.html"), Path::new("index.html")),
            "../index.html"
        );
    }

    #[test]
    fn frame_rule_joins_selectors() {
        assert_eq!(
            frame_rule(&["place".into(), "rect".into()]),
            "#show selector(place).or(selector(rect)): html.frame;"
        );
    }

    #[test]
    fn zip_has_signature_and_directory() {
        let zip = zip_store(&[("a.html".into(), b"hi".to_vec())]);
        assert_eq!(&zip[..4], b"PK\x03\x04");
        assert_eq!(&zip[zip.len() - 22..zip.len() - 18], b"PK\x05\x06");
    }
}
