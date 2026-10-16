//! Заметка отчёта об импорте (фича 021, FR-012, экран C3).
//!
//! Отчёт — обычная заметка Typst в папке назначения: что перенесено чисто, что
//! с потерями и в каком месте, что пропущено и почему. Тексты приходят из
//! переводов интерфейса (`ImportJob::texts`), а не живут в ядре; слова по
//! умолчанию нужны только тестам и оболочкам, которым переводы не передали.
//! Всё, что попадает в заметку из файлов пользователя, экранируется — отчёт
//! собирается всегда.

use serde::{Deserialize, Serialize};

use super::markdown::escape_markup;
use super::{LossKind, Outcome, ReportEntry, SkipReason};

/// Тексты отчёта.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(default)]
pub struct ReportTexts {
    /// Имя файла отчёта без расширения.
    pub file_name: String,
    pub title: String,
    pub source: String,
    pub destination: String,
    pub summary: String,
    pub clean: String,
    pub lossy: String,
    pub skipped: String,
    pub resumed: String,
    pub cancelled: String,
    pub lossy_heading: String,
    pub skipped_heading: String,
    pub renamed_heading: String,
    /// Шаблон «строка {n}».
    pub line: String,
    pub loss_html: String,
    pub loss_embed: String,
    pub loss_plugin_block: String,
    pub loss_diagram: String,
    pub loss_math: String,
    pub loss_section_link: String,
    pub loss_block_link: String,
    pub loss_missing_target: String,
    pub loss_ambiguous_target: String,
    pub loss_properties: String,
    pub loss_syntax: String,
    pub skip_too_large: String,
    pub skip_not_utf8: String,
    pub skip_other_file: String,
    pub skip_exists_skipped: String,
    pub skip_unreadable: String,
    pub skip_write_failed: String,
}

impl Default for ReportTexts {
    fn default() -> Self {
        let s = |text: &str| text.to_owned();
        ReportTexts {
            file_name: s("Import report"),
            title: s("Import report"),
            source: s("Source"),
            destination: s("Destination"),
            summary: s("Summary"),
            clean: s("Imported cleanly"),
            lossy: s("Imported with losses"),
            skipped: s("Skipped"),
            resumed: s("Imported earlier"),
            cancelled: s("The import was cancelled before it finished."),
            lossy_heading: s("Notes with losses"),
            skipped_heading: s("Skipped files"),
            renamed_heading: s("Renamed"),
            line: s("line {n}"),
            loss_html: s("HTML left as source text"),
            loss_embed: s("Embed replaced or left as source text"),
            loss_plugin_block: s("Plugin block left as source text"),
            loss_diagram: s("Diagram left as source text"),
            loss_math: s("Formula left as source text"),
            loss_section_link: s("Link to a section leads to the whole note"),
            loss_block_link: s("Link to a block leads to the whole note"),
            loss_missing_target: s("Link target not found"),
            loss_ambiguous_target: s("Several notes share the name; the nearest was used"),
            loss_properties: s("Properties header left as a comment"),
            loss_syntax: s("Note left as source text"),
            skip_too_large: s("too large"),
            skip_not_utf8: s("not UTF-8"),
            skip_other_file: s("not a note"),
            skip_exists_skipped: s("already exists"),
            skip_unreadable: s("could not be read"),
            skip_write_failed: s("could not be written"),
        }
    }
}

impl ReportTexts {
    fn loss(&self, kind: LossKind) -> &str {
        match kind {
            LossKind::Html => &self.loss_html,
            LossKind::Embed => &self.loss_embed,
            LossKind::PluginBlock => &self.loss_plugin_block,
            LossKind::Diagram => &self.loss_diagram,
            LossKind::Math => &self.loss_math,
            LossKind::SectionLink => &self.loss_section_link,
            LossKind::BlockLink => &self.loss_block_link,
            LossKind::MissingTarget => &self.loss_missing_target,
            LossKind::AmbiguousTarget => &self.loss_ambiguous_target,
            LossKind::Properties => &self.loss_properties,
            LossKind::Syntax => &self.loss_syntax,
        }
    }

    fn skip(&self, reason: SkipReason) -> &str {
        match reason {
            SkipReason::TooLarge => &self.skip_too_large,
            SkipReason::NotUtf8 => &self.skip_not_utf8,
            SkipReason::OtherFile => &self.skip_other_file,
            SkipReason::ExistsSkipped => &self.skip_exists_skipped,
            SkipReason::Unreadable => &self.skip_unreadable,
            SkipReason::WriteFailed => &self.skip_write_failed,
        }
    }
}

/// Счётчики итога.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Totals {
    pub clean: u32,
    pub lossy: u32,
    pub skipped: u32,
    pub resumed: u32,
}

/// Текст заметки отчёта. `entries` — все файлы с потерями, пропущенные и
/// переименованные; чисто перенесённые без переименования входят в итог.
pub fn build(
    texts: &ReportTexts,
    source_hint: &str,
    destination: &str,
    report_path: &str,
    entries: &[ReportEntry],
    totals: Totals,
    cancelled: bool,
) -> String {
    let e = |text: &str| escape_markup(text, false);
    let mut out = String::new();
    out.push_str(&format!("= {}\n\n", e(&texts.title)));
    out.push_str(&format!("{}: {}\\\n", e(&texts.source), e(source_hint)));
    let shown_destination = if destination.is_empty() {
        "/"
    } else {
        destination
    };
    out.push_str(&format!(
        "{}: {}\n\n",
        e(&texts.destination),
        e(shown_destination)
    ));
    if cancelled {
        out.push_str(&format!("_{}_\n\n", e(&texts.cancelled)));
    }

    out.push_str(&format!("== {}\n\n", e(&texts.summary)));
    out.push_str(&format!("- {}: {}\n", e(&texts.clean), totals.clean));
    out.push_str(&format!("- {}: {}\n", e(&texts.lossy), totals.lossy));
    out.push_str(&format!("- {}: {}\n", e(&texts.skipped), totals.skipped));
    if totals.resumed > 0 {
        out.push_str(&format!("- {}: {}\n", e(&texts.resumed), totals.resumed));
    }
    out.push('\n');

    let lossy: Vec<&ReportEntry> = entries
        .iter()
        .filter(|entry| entry.outcome == Outcome::Lossy)
        .collect();
    if !lossy.is_empty() {
        out.push_str(&format!("== {}\n\n", e(&texts.lossy_heading)));
        for entry in lossy {
            let mut line = format!("- {}", note_reference(report_path, entry));
            for loss in &entry.losses {
                let place = texts.line.replace("{n}", &loss.line.to_string());
                line.push_str(&format!(
                    "\n  - {}: {}",
                    e(&place),
                    e(texts.loss(loss.kind))
                ));
            }
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }

    let skipped: Vec<&ReportEntry> = entries
        .iter()
        .filter(|entry| entry.outcome == Outcome::Skipped)
        .collect();
    if !skipped.is_empty() {
        out.push_str(&format!("== {}\n\n", e(&texts.skipped_heading)));
        for entry in skipped {
            let reason = entry
                .skip_reason
                .map(|reason| texts.skip(reason).to_owned())
                .unwrap_or_default();
            out.push_str(&format!("- {} — {}\n", e(&entry.source), e(&reason)));
        }
        out.push('\n');
    }

    let renamed: Vec<&ReportEntry> = entries
        .iter()
        .filter(|entry| entry.renamed.is_some() && entry.outcome != Outcome::Skipped)
        .collect();
    if !renamed.is_empty() {
        out.push_str(&format!("== {}\n\n", e(&texts.renamed_heading)));
        for entry in renamed {
            out.push_str(&format!(
                "- {} → {}\n",
                e(&entry.source),
                e(entry.renamed.as_deref().unwrap_or(""))
            ));
        }
        out.push('\n');
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
}

/// Ссылка vault на перенесённую заметку; если записать её нельзя — имя текстом.
fn note_reference(report_path: &str, entry: &ReportEntry) -> String {
    let shown = escape_markup(&entry.source, false);
    let Some(destination) = &entry.destination else {
        return shown;
    };
    let link = crate::graph::link_text(
        std::path::Path::new(report_path),
        std::path::Path::new(destination),
    );
    match link {
        Some(link) => {
            let inner = &link[2..link.len() - 2];
            let label: String = entry.source.replace(['[', ']', '|', '\n'], " ");
            format!("[[{inner}|{}]]", escape_markup(label.trim(), false))
        }
        None => shown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::Loss;

    fn entry(source: &str, destination: Option<&str>, outcome: Outcome) -> ReportEntry {
        ReportEntry {
            source: source.into(),
            destination: destination.map(str::to_owned),
            outcome,
            losses: Vec::new(),
            skip_reason: None,
            renamed: None,
        }
    }

    #[test]
    fn the_report_lists_losses_skips_and_renames_and_parses() {
        let mut lossy = entry(
            "a/Заметка #1.md",
            Some("Импорт/a/Заметка 1.typ"),
            Outcome::Lossy,
        );
        lossy.losses = vec![
            Loss {
                kind: LossKind::Html,
                line: 3,
            },
            Loss {
                kind: LossKind::Math,
                line: 9,
            },
        ];
        lossy.renamed = Some("Импорт/a/Заметка 1.typ".into());
        let mut skipped = entry("big.md", None, Outcome::Skipped);
        skipped.skip_reason = Some(SkipReason::TooLarge);
        let text = build(
            &ReportTexts::default(),
            "Мой *vault*",
            "Импорт",
            "Импорт/Import report.typ",
            &[lossy, skipped],
            Totals {
                clean: 5,
                lossy: 1,
                skipped: 1,
                resumed: 2,
            },
            true,
        );
        assert!(text.contains("line 3"));
        assert!(text.contains("HTML left as source text"));
        assert!(text.contains("big.md — too large"));
        assert!(text.contains("Imported earlier: 2"));
        assert!(text.contains("cancelled"));
        assert!(!typst::syntax::parse(&text).diagnosis().errors, "{text}");
        // Ссылка на заметку читается графом ссылок.
        assert_eq!(
            crate::graph::links_in(std::path::Path::new("Импорт/Import report.typ"), &text),
            [std::path::PathBuf::from("Импорт/a/Заметка 1.typ")]
        );
    }
}
