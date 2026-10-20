//! Цель перехода для ссылки под кареткой (research.md §7, FR-005).
//!
//! `@имя`: метка в заметке → в связанных файлах → ключ источника →
//! подтверждена сборкой → нет. `[[цель]]`: разрешение 013 и индекс заметок.
//! Повторная метка ведёт к первому объявлению.

use std::path::Path;

use super::{DefinitionOutcome, DefinitionRequest, NoDefinition};
use crate::graph::{link_sites, resolve_link_target};
use crate::preview::DiagnosticLocation;
use crate::text_offsets::byte_at;
use crate::vault::backend::VaultBackend;

pub async fn definition_at<B: VaultBackend>(
    backend: &B,
    request: &DefinitionRequest,
) -> DefinitionOutcome {
    let text = &request.text;
    let structure = crate::structure::parse(text);
    // Каретка сразу после последнего знака тоже считается на ссылке.
    let reference = structure
        .refs
        .iter()
        .find(|site| site.place.from <= request.offset && request.offset <= site.place.to);
    if let Some(reference) = reference {
        return label_target(request, &structure, &reference.target);
    }

    let caret = byte_at(text, request.offset as usize);
    let link = link_sites(text)
        .into_iter()
        .find(|site| site.byte_start <= caret && caret <= site.byte_end);
    let Some(link) = link else {
        return nothing(NoDefinition::NoReferenceAtCaret);
    };
    let Some(target) = resolve_link_target(Path::new(&request.note_path), &link.raw) else {
        return nothing(NoDefinition::OutsideVault { target: link.raw });
    };
    if !backend.list().await.iter().any(|note| note.path == target) {
        return nothing(NoDefinition::NoteMissing { target: link.raw });
    }
    let path = target.to_string_lossy().replace('\\', "/");
    DefinitionOutcome::Place {
        location: DiagnosticLocation {
            is_open_note: path == request.note_path,
            path,
            line: 1,
            column: 1,
            byte_start: 0,
            byte_end: 0,
        },
    }
}

fn label_target(
    request: &DefinitionRequest,
    structure: &crate::structure::NoteStructure,
    name: &str,
) -> DefinitionOutcome {
    if let Some(label) = structure.labels.iter().find(|label| label.name == name) {
        let lines = typst::syntax::Lines::new(request.text.as_str());
        let byte = |utf16: u32| {
            lines
                .utf16_to_byte(utf16 as usize)
                .unwrap_or(request.text.len()) as u32
        };
        return DefinitionOutcome::Place {
            location: DiagnosticLocation {
                path: request.note_path.clone(),
                is_open_note: true,
                line: label.place.line,
                column: label.place.column,
                byte_start: byte(label.place.from),
                byte_end: byte(label.place.to),
            },
        };
    }
    for file in &request.linked.files {
        if let Some(label) = file.labels.iter().find(|label| label.name == name) {
            return DefinitionOutcome::Place {
                location: DiagnosticLocation {
                    path: file.path.clone(),
                    is_open_note: false,
                    line: label.line,
                    column: label.column,
                    byte_start: label.byte_start,
                    byte_end: label.byte_end,
                },
            };
        }
    }
    if request.sources.iter().any(|key| key == name) {
        return DefinitionOutcome::Source {
            key: name.to_owned(),
        };
    }
    let name = name.to_owned();
    if request
        .evidence
        .as_ref()
        .is_some_and(|evidence| evidence.count(&name) > 0)
    {
        return nothing(NoDefinition::LabelFromBuild { name });
    }
    nothing(NoDefinition::LabelMissing { name })
}

fn nothing(reason: NoDefinition) -> DefinitionOutcome {
    DefinitionOutcome::Nothing { reason }
}
