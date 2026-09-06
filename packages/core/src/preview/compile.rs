//! Сборка документа (contracts/compile-engine.md).
//!
//! Синхронная фаза: работает над готовым снимком и к файловой системе не
//! обращается. Асинхронная фаза резолва, чередование фаз и предел времени —
//! задачи T014…T016 и T023; здесь пока прямая сборка из текста редактора,
//! которой достаточно для односоставной заметки.

use serde::{Deserialize, Serialize};
use typst::foundations::Bytes;
use typst_layout::PagedDocument;

use super::diagnostics::{sort_by_location, translate, Diagnostic};
use super::fonts::FontSet;
use super::pages::{render_pages, RenderedPage};
use super::snapshot::{DependencySnapshot, SnapshotError};
use super::world::PreviewWorld;

/// Чем закончилась сборка (data-model.md «CompileOutcome»).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum CompileOutcome {
    /// Документ собран. Диагностики при этом могут содержать предупреждения,
    /// но **ни одной** ошибки (запрещённое состояние из data-model.md).
    Succeeded,
    /// Документ не собран; среди диагностик есть хотя бы одна ошибка.
    Failed,
}

/// Свёрстанный вид (data-model.md «CompiledDocument»).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CompiledDocument {
    /// Версия текста, из которой собран результат. Результат с версией меньше
    /// уже показанной UI обязан отбросить (FR-009).
    pub revision: u64,
    pub note_path: String,
    pub outcome: CompileOutcome,
    /// **Все** страницы документа по порядку, каждая со своим `digest`. Ядро
    /// не фильтрует: пропуск неизменившихся — забота UI (research.md §5).
    pub pages: Vec<RenderedPage>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Собирает заметку из текста редактора.
///
/// Текст приходит из буфера, а не с диска: иначе требование «без кнопки
/// сохранить» невыполнимо (FR-002, FR-006). Файл vault при этом не читается и
/// не изменяется.
pub fn compile_note(
    note_path: &str,
    source_text: &str,
    revision: u64,
    fonts: FontSet,
) -> Result<CompiledDocument, SnapshotError> {
    let main = DependencySnapshot::file_id(note_path)?;
    let mut snapshot = DependencySnapshot::new();
    snapshot.insert(main, Bytes::new(source_text.as_bytes().to_vec()));

    let world = PreviewWorld::new(main, snapshot, fonts.into_fonts());
    let compiled = typst::compile::<PagedDocument>(&world);

    let mut diagnostics: Vec<Diagnostic> = compiled
        .warnings
        .iter()
        .map(|d| translate(&world, main, d))
        .collect();

    let (outcome, pages) = match compiled.output {
        Ok(document) => (CompileOutcome::Succeeded, render_pages(&document)),
        Err(errors) => {
            diagnostics.extend(errors.iter().map(|d| translate(&world, main, d)));
            (CompileOutcome::Failed, Vec::new())
        }
    };

    sort_by_location(&mut diagnostics);

    Ok(CompiledDocument {
        revision,
        note_path: note_path.to_owned(),
        outcome,
        pages,
        diagnostics,
    })
}
