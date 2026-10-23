//! Дополнение меток и ссылок (фича 018): список предложений по `@` и `[[`,
//! переход к определению и отметки ссылок в никуда.
//!
//! Разбора здесь нет: метки и ссылки `@имя` — из `structure` (015), записи
//! `[[…]]` — из `graph` (013), режим места каретки — тем же разбором Typst.
//! Все функции, кроме чтения связанных файлов и списка заметок, чистые;
//! `backend` нужен только этим двум вопросам (contracts/core-completion-api.md).

pub mod context;
pub mod definition;
pub mod labels;
pub mod linked;
pub mod marks;
pub mod notes;

#[cfg(target_arch = "wasm32")]
pub mod web;

use serde::{Deserialize, Serialize};

use crate::preview::evidence::BuildEvidence;
use crate::preview::DiagnosticLocation;
use crate::structure::LabelKind;
use crate::vault::backend::VaultBackend;

pub use context::{at as context_at, CaretContext};
pub use definition::definition_at;
pub use linked::linked_index;
pub use marks::{label_marks, reference_marks};

/// Строк в одной группе списка (research.md §5).
pub const GROUP_LIMIT: usize = 50;

// ── Связанные файлы ─────────────────────────────────────────────────────────

/// Метки файлов `.typ`, которые подключает заметка (research.md §3).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LinkedIndex {
    pub note_path: String,
    /// По алфавиту путей; сама заметка не входит.
    pub files: Vec<LinkedFile>,
    /// Пути, которые не прочитались: пока они есть, «метки нет» не ставится.
    pub unreadable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LinkedFile {
    pub path: String,
    pub labels: Vec<LinkedLabel>,
}

/// Метка связанного файла: место — в файле, а не в тексте редактора.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LinkedLabel {
    pub name: String,
    pub kind: LabelKind,
    pub text: String,
    pub line: u32,
    pub column: u32,
    pub byte_start: u32,
    pub byte_end: u32,
    pub top_level: bool,
}

/// Ключ источника 017 со строкой «авторы · год» — 018 показывает как есть.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SourceKey {
    pub key: String,
    pub summary: String,
}

// ── Список предложений ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CompletionRequest {
    pub note_path: String,
    /// Буфер редактора, а не файл на диске (FR-008).
    pub text: String,
    /// Каретка, UTF-16.
    pub offset: u32,
    /// Вызов командой, а не набором (FR-009).
    pub manual: bool,
    pub linked: LinkedIndex,
    pub sources: Vec<SourceKey>,
}

/// Почему предлагать нечего (research.md §2). При наборе не показывается.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum NoCompletionReason {
    NoTrigger,
    InCode,
    InString,
    InComment,
    InRaw,
    InMath,
    InAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum CompletionTarget {
    Label,
    Note,
    None { reason: NoCompletionReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum GroupKind {
    Labels,
    Sources,
    Notes,
    /// Разделы заметки цели встраивания (031).
    Sections,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ItemDetail {
    Label {
        label_kind: LabelKind,
        text: String,
        /// Есть у метки связанного файла.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(target_arch = "wasm32", tsify(optional))]
        file: Option<String>,
        duplicate: bool,
    },
    Source {
        summary: String,
    },
    Note {
        folder: String,
        same_folder: bool,
    },
    /// Раздел заметки цели встраивания (031): уровень заголовка.
    Section {
        level: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CompletionItem {
    /// Имя метки, ключ или имя заметки.
    pub label: String,
    /// Отрезки совпадения в `label`, UTF-16.
    pub matched: Vec<(u32, u32)>,
    pub detail: ItemDetail,
    /// Что встанет на место `from..to`.
    pub insert: String,
    /// Итоговая запись для подвала: `@рис-схема`, `[[План]]`.
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CompletionGroup {
    pub kind: GroupKind,
    /// Сколько подошло всего; строк — не больше [`GROUP_LIMIT`].
    pub total: u32,
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CompletionList {
    pub target: CompletionTarget,
    /// Набранная часть, которую заменит выбор (UTF-16).
    pub from: u32,
    pub to: u32,
    pub query: String,
    /// Пустые группы не отдаются; при `None` групп нет.
    pub groups: Vec<CompletionGroup>,
}

impl CompletionList {
    fn nothing(reason: NoCompletionReason, offset: u32) -> Self {
        Self {
            target: CompletionTarget::None { reason },
            from: offset,
            to: offset,
            query: String::new(),
            groups: Vec::new(),
        }
    }
}

/// Список предложений у каретки (contracts/core-completion-api.md).
/// `backend` спрашивается только о списке заметок и только для `[[`.
pub async fn completions<B: VaultBackend>(
    backend: &B,
    request: &CompletionRequest,
) -> CompletionList {
    // Один разбор на контекст каретки и на метки заметки.
    let root = typst::syntax::parse(&request.text);
    let caret = match context::at_in(&root, &request.text, request.offset, request.manual) {
        Ok(caret) => caret,
        Err(reason) => return CompletionList::nothing(reason, request.offset),
    };
    let (target, groups) = match caret.target {
        context::Trigger::Label => (
            CompletionTarget::Label,
            labels::candidates_in(
                &crate::structure::of_tree(&root, &request.text),
                &request.text,
                request.offset,
                &caret.query,
                &request.linked,
                &request.sources,
            ),
        ),
        context::Trigger::Note { closed } => {
            let notes = backend.list().await;
            (
                CompletionTarget::Note,
                notes::candidates(&request.note_path, &notes, &caret.query, closed),
            )
        }
        // Встраивание (031): заметки до `#`, разделы и метки цели после.
        context::Trigger::Embed { part: false } => {
            let notes = backend.list().await;
            (
                CompletionTarget::Note,
                notes::candidates(&request.note_path, &notes, &caret.query, true),
            )
        }
        context::Trigger::Embed { part: true } => (
            CompletionTarget::Label,
            embed_parts(backend, &request.note_path, &caret.embed_note, &caret.query).await,
        ),
    };
    CompletionList {
        target,
        from: caret.from,
        to: caret.to,
        query: caret.query,
        groups,
    }
}

// ── Переход к определению ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DefinitionRequest {
    pub note_path: String,
    pub text: String,
    pub offset: u32,
    pub linked: LinkedIndex,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub evidence: Option<BuildEvidence>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum NoDefinition {
    NoReferenceAtCaret,
    LabelMissing {
        name: String,
    },
    /// Метка есть в собранном документе, но места в тексте у неё нет.
    LabelFromBuild {
        name: String,
    },
    NoteMissing {
        target: String,
    },
    OutsideVault {
        target: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum DefinitionOutcome {
    Place { location: DiagnosticLocation },
    Source { key: String },
    Nothing { reason: NoDefinition },
}

// ── Отметки ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MarksRequest {
    pub note_path: String,
    pub text: String,
    pub linked: LinkedIndex,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub evidence: Option<BuildEvidence>,
    pub sources: Vec<String>,
    /// Диапазоны, уже отмеченные 017 (UTF-16).
    pub claimed: Vec<(u32, u32)>,
    /// Сборка текущего текста ещё не вернулась.
    pub held: bool,
    /// Имена, отмеченные «метки нет» в прошлом ответе.
    pub previously_missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DuplicatePlace {
    /// Есть у объявления в связанном файле.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub path: Option<String>,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum MarkKind {
    MissingLabel {
        name: String,
    },
    MissingNote {
        target: String,
        /// Путь в vault, по которому «Создать заметку» её создаст.
        create_path: String,
    },
    LinkOutsideVault {
        target: String,
    },
    /// Встраивание (031): заметка есть, а раздела или метки в ней нет.
    MissingEmbedPart {
        target: String,
        part: String,
    },
    DuplicateLabel {
        name: String,
        others: Vec<DuplicatePlace>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ReferenceMark {
    pub kind: MarkKind,
    /// UTF-16 от начала текста редактора.
    pub from: u32,
    pub to: u32,
    pub line: u32,
}

// ── Общее ───────────────────────────────────────────────────────────────────

/// Ярус совпадения (research.md §5): начало имени → начало части после
/// `-`, `_`, `:`, `.` → прочее вхождение; `None` — не подходит. Сравнение без
/// учёта регистра; `matched` — отрезок в UTF-16 исходного имени.
pub(crate) fn match_tier(name: &str, query: &str) -> Option<(u8, Vec<(u32, u32)>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    let lower_name = name.to_lowercase();
    let lower_query = query.to_lowercase();
    // Смещение в нижнем регистре переводится в исходное имя по знакам: у
    // кириллицы и латиницы число знаков при смене регистра не меняется.
    let found = lower_name.find(&lower_query)?;
    let start_chars = lower_name[..found].chars().count();
    let query_chars = lower_query.chars().count();
    let start: u32 = name
        .chars()
        .take(start_chars)
        .map(char::len_utf16)
        .sum::<usize>() as u32;
    let length: u32 = name
        .chars()
        .skip(start_chars)
        .take(query_chars)
        .map(char::len_utf16)
        .sum::<usize>() as u32;
    let tier = if found == 0 {
        0
    } else if lower_name
        .match_indices(&lower_query)
        .any(|(at, _)| lower_name[..at].ends_with(['-', '_', ':', '.', ' ', '/']))
    {
        1
    } else {
        2
    };
    Some((tier, vec![(start, start + length)]))
}

/// Разделы и метки заметки цели встраивания (031, US1 AS4): вставка —
/// текст заголовка или `<метка>`.
async fn embed_parts<B: VaultBackend>(
    backend: &B,
    note_path: &str,
    note: &str,
    query: &str,
) -> Vec<CompletionGroup> {
    let Some(path) = crate::graph::resolve_link_target(std::path::Path::new(note_path), note)
    else {
        return Vec::new();
    };
    let Ok(text) = backend.read(&path).await else {
        return Vec::new();
    };
    let structure = crate::structure::parse(&text);
    let preview = |part: &str| format!("#embed(\"{note}#{part}\")");
    let sections: Vec<CompletionItem> = structure
        .headings
        .iter()
        .filter_map(|heading| {
            let (_, matched) = match_tier(&heading.text, query)?;
            Some(CompletionItem {
                label: heading.text.clone(),
                matched,
                detail: ItemDetail::Section {
                    level: heading.level,
                },
                insert: heading.text.clone(),
                preview: preview(&heading.text),
            })
        })
        .collect();
    let labels: Vec<CompletionItem> = structure
        .labels
        .iter()
        .filter_map(|label| {
            let (_, matched) = match_tier(&label.name, query)?;
            let insert = format!("<{}>", label.name);
            Some(CompletionItem {
                label: label.name.clone(),
                matched,
                detail: ItemDetail::Label {
                    label_kind: label.kind,
                    text: label.text.clone(),
                    file: None,
                    duplicate: false,
                },
                preview: preview(&insert),
                insert,
            })
        })
        .collect();
    let mut groups = Vec::new();
    labels::push_group(&mut groups, GroupKind::Sections, sections);
    labels::push_group(&mut groups, GroupKind::Labels, labels);
    groups
}
