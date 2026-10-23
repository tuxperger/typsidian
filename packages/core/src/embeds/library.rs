//! Файл библиотеки `embed.typ` и строка `#import` (фича 031, research.md §1) —
//! по схеме библиотеки выборки 020.

use std::path::Path;

use serde::{Deserialize, Serialize};
use typst::syntax::{ast, LinkedNode};

use crate::elements::edit::TextEdit;
use crate::elements::utf16_at;
use crate::properties::LibraryStatus;
use crate::vault::backend::{VaultBackend, WriteError};

/// Путь библиотеки внутри vault.
pub const LIBRARY_PATH: &str = ".typsidian/lib/embed.typ";
/// Путь для `#import`: от корня vault.
pub const LIBRARY_IMPORT: &str = "/.typsidian/lib/embed.typ";
/// Версия текста библиотеки (первая строка файла).
pub const LIBRARY_VERSION: u32 = 1;
/// Текст `embed.typ`.
pub const LIBRARY_TEXT: &str = include_str!("embed.typ");

/// Файл библиотеки: путь и текст.
pub fn embed_library() -> (&'static str, &'static str) {
    (LIBRARY_PATH, LIBRARY_TEXT)
}

fn version_of(text: &str) -> Option<u32> {
    text.lines()
        .next()?
        .trim()
        .strip_prefix("// typsidian-embed v")?
        .trim()
        .parse()
        .ok()
}

/// Кладёт `.typsidian/lib/embed.typ` в vault, если его нет; правку руками не
/// затирает.
pub async fn ensure_embed_library<B: VaultBackend>(
    backend: &mut B,
) -> Result<LibraryStatus, WriteError> {
    let path = Path::new(LIBRARY_PATH);
    if let Ok(existing) = backend.read(path).await {
        return Ok(match version_of(&existing) {
            Some(v) if v > LIBRARY_VERSION => LibraryStatus::UpToDate,
            Some(v) if v == LIBRARY_VERSION && existing == LIBRARY_TEXT => LibraryStatus::UpToDate,
            _ => LibraryStatus::Customized,
        });
    }
    backend.ensure_folder(Path::new(".typsidian/lib")).await?;
    backend.write_bytes(path, LIBRARY_TEXT.as_bytes()).await?;
    Ok(LibraryStatus::Written)
}

/// Правка, добавляющая `#import` библиотеки, если его ещё нет: в начало
/// заметки, после записи свойств 020.
pub fn embed_import_edits(text: &str) -> Vec<TextEdit> {
    let root = typst::syntax::parse(text);
    let linked = LinkedNode::new(&root);
    let imported = linked.children().any(|node| {
        node.cast::<ast::ModuleImport>().is_some_and(
            |import| matches!(import.source(), ast::Expr::Str(s) if s.get() == LIBRARY_IMPORT),
        )
    });
    if imported {
        return Vec::new();
    }
    let at = crate::properties::import_position(&linked, text);
    let at16 = utf16_at(text, at);
    vec![TextEdit {
        from: at16,
        to: at16,
        insert: format!("#import \"{LIBRARY_IMPORT}\": embed\n"),
    }]
}

/// Ответ команды «Встроить заметку»: правки текста и что стало с библиотекой.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct EmbedInsertion {
    /// Правки в координатах исходного текста, без пересечений (одна транзакция
    /// редактора).
    pub edits: Vec<TextEdit>,
    /// Каретка после правок — внутри кавычек `embed("")`, UTF-16.
    pub caret: u32,
    pub library: LibraryStatus,
}

/// Команда «Встроить заметку» (research.md §6): библиотека в vault, `#import`
/// при отсутствии и `#embed("")` у каретки `offset` (UTF-16).
pub async fn insert_embed<B: VaultBackend>(
    backend: &mut B,
    text: &str,
    offset: u32,
) -> Result<EmbedInsertion, WriteError> {
    let library = ensure_embed_library(backend).await?;
    let import = embed_import_edits(text);
    let shift: u32 = import
        .iter()
        .filter(|edit| edit.from as u32 <= offset)
        .map(|edit| edit.insert.encode_utf16().count() as u32)
        .sum();
    let call = "#embed(\"\")";
    let mut edits = import;
    edits.push(TextEdit {
        from: offset as usize,
        to: offset as usize,
        insert: call.to_owned(),
    });
    Ok(EmbedInsertion {
        edits,
        caret: offset + shift + "#embed(\"".encode_utf16().count() as u32,
        library,
    })
}
