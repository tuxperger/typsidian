//! Сбор сведений о страницах при сборке (research.md §1–§2).
//!
//! Вызывается из `preview::compile` при `Render::Pages`, пока документ и мир
//! сборки ещё живы: в web документ существует только в воркере, поэтому всё
//! нужное показу приезжает готовым вместе со страницами.

use std::collections::{BTreeMap, HashMap};

use typst::foundations::{Label, Selector, Value};
use typst::introspection::{Introspector, MetadataElem};
use typst::layout::{Frame, FrameItem};
use typst::syntax::{FileId, Lines, LinkedNode, Source, Span};
use typst::utils::PicoStr;
use typst::World;
use typst_layout::PagedDocument;

use super::{PageSlideInfo, SlidePlace};

/// Метка, под которой пакеты презентаций выгружают разметку для pdfpc.
const PDFPC_LABEL: &str = "pdfpc";

/// Сведения о каждой странице документа, по порядку страниц.
pub(crate) fn collect(
    document: &PagedDocument,
    world: &dyn World,
    main: FileId,
    source: &str,
) -> Vec<PageSlideInfo> {
    let lines = Lines::new(source);
    let marks = pdfpc_marks(document);
    // Начало каждого узла главного файла — одним обходом дерева разбора.
    // `world.range(span)` на каждый глиф шёл бы через `World::source`, а
    // `PreviewWorld::source` строит `Source` заново: разбор всего документа на
    // глиф, 20 с на 100 страницах. И `Source::range` ищет узел от корня — на
    // глиф это тоже долгий проход (019 T030, research.md §11).
    let starts = world
        .source(main)
        .map(|main_source| node_starts(&main_source))
        .unwrap_or_default();
    document
        .pages()
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let mut first = None;
            first_byte(&page.frame, &starts, main, &mut first);
            let mark = marks.get(&index);
            PageSlideInfo {
                page_index: index as u32,
                source_start: first.map(|byte| place(&lines, byte)),
                logical_slide: mark.and_then(|m| m.logical_slide),
                notes: mark.and_then(|m| (!m.notes.is_empty()).then(|| m.notes.join("\n\n"))),
            }
        })
        .collect()
}

/// Место по байту главного файла. Строка и колонка — с нуля.
fn place(lines: &Lines<&str>, byte: usize) -> SlidePlace {
    let (line, column) = lines.byte_to_line_column(byte).unwrap_or((0, 0));
    SlidePlace {
        offset: lines.byte_to_utf16(byte).unwrap_or(0) as u32,
        line: line as u32,
        column: column as u32,
    }
}

/// Наименьший байтовый старт спанов глифов главного файла на кадре. Спаны
/// других файлов и оторванные (`detached`) пропускаются.
fn first_byte(
    frame: &Frame,
    starts: &HashMap<Span, usize>,
    main: FileId,
    first: &mut Option<usize>,
) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Group(group) => first_byte(&group.frame, starts, main, first),
            FrameItem::Text(text) => {
                for glyph in &text.glyphs {
                    let span = glyph.span.0;
                    if span.id() != Some(main) {
                        continue;
                    }
                    if let Some(&start) = starts.get(&span) {
                        *first = Some(first.map_or(start, |f| f.min(start)));
                    }
                }
            }
            FrameItem::Shape(..)
            | FrameItem::Image(..)
            | FrameItem::Link(..)
            | FrameItem::Tag(..) => {}
        }
    }
}

/// То, что разметка `<pdfpc>` говорит об одной странице.
#[derive(Default)]
struct PageMarks {
    notes: Vec<String>,
    logical_slide: Option<u32>,
}

/// Разметка `<pdfpc>` по номерам страниц (с нуля).
fn pdfpc_marks(document: &PagedDocument) -> BTreeMap<usize, PageMarks> {
    let mut marks: BTreeMap<usize, PageMarks> = BTreeMap::new();
    let Some(label) = Label::new(PicoStr::intern(PDFPC_LABEL)) else {
        return marks;
    };
    let introspector = document.introspector();
    for content in introspector.query(&Selector::Label(label)) {
        let Some(meta) = content.to_packed::<MetadataElem>() else {
            continue;
        };
        let Some(location) = content.location() else {
            continue;
        };
        let Some(position) = introspector.position(location) else {
            continue;
        };
        let page = position.page.get() - 1;
        let Value::Dict(dict) = &meta.value else {
            continue;
        };
        let (Ok(Value::Str(kind)), Ok(value)) = (dict.get("t"), dict.get("v")) else {
            continue;
        };
        let entry = marks.entry(page).or_default();
        match (kind.as_str(), value) {
            ("Note", Value::Str(text)) => entry.notes.push(text.to_string()),
            ("Note", Value::Content(body)) => entry.notes.push(body.plain_text().to_string()),
            ("LogicalSlide", Value::Int(n)) => {
                if let Ok(n) = u32::try_from(*n) {
                    entry.logical_slide = Some(n);
                }
            }
            _ => {}
        }
    }
    marks
}

/// Начало в байтах каждого узла дерева разбора, по его спану.
fn node_starts(main_source: &Source) -> HashMap<Span, usize> {
    let mut starts = HashMap::new();
    let mut stack = vec![LinkedNode::new(main_source.root())];
    while let Some(node) = stack.pop() {
        starts.insert(node.span(), node.offset());
        stack.extend(node.children());
    }
    starts
}
