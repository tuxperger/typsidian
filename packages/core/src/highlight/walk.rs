//! Обход дерева разбора: листья → фрагменты, окна, склейка, упаковка.

use std::ops::Range;

use typst::syntax::{parse, LinkedNode, Source, SyntaxKind, SyntaxNode};

use super::roles::{role_of, NodeRole};
use super::{HighlightSpans, HighlightWindow, StyleFlags, SyntaxRole};
use crate::text_offsets::{byte_at, Utf16Index};

/// Раскраска без состояния: разбор всего текста.
pub(super) fn highlight(text: &str, windows: &[HighlightWindow]) -> HighlightSpans {
    let root = parse(text);
    let ranges = byte_windows(text, windows, |utf16| byte_at(text, utf16));
    let spans = collect(text, &root, &ranges);
    let mut index = Utf16Index::new(text);
    pack(spans, |byte| index.at(byte))
}

/// Раскраска уже разобранного текста сессии: дерево — инкрементальное, смещения —
/// через индекс строк `Source`, без прохода по тексту.
pub(super) fn highlight_source(source: &Source, windows: &[HighlightWindow]) -> HighlightSpans {
    let text = source.text();
    let lines = source.lines();
    let ranges = byte_windows(text, windows, |utf16| {
        lines
            .utf16_to_byte(utf16)
            .unwrap_or_else(|| byte_at(text, utf16))
    });
    let spans = collect(text, source.root(), &ranges);
    pack(spans, |byte| {
        lines
            .byte_to_utf16(byte)
            .unwrap_or_else(|| lines.len_utf16())
    })
}

/// Фрагменты в байтах. `root` — разбор всего `text`.
fn collect(text: &str, root: &SyntaxNode, windows: &[Range<usize>]) -> Vec<ByteSpan> {
    let mut spans = Vec::new();

    // Незакрытая скобка, `$` или строка у Typst тянут свой режим через пустые
    // строки до конца заметки: следующий заголовок становится кодом. Спека
    // держит такую пару в пределах абзаца (FR-006), поэтому разбор обрезается
    // концом абзаца с незакрытой парой, а остаток разбирается заново — как если
    // бы пару закрыли. Корректного текста обрезка не касается: она срабатывает
    // только на ошибке «незакрыто».
    let mut base = 0;
    let mut tail;
    let mut root = root;
    loop {
        let segment = &text[base..];
        let linked = LinkedNode::new(root);
        let fence = first_unclosed(&linked).and_then(|at| paragraph_end(segment, at));
        let mut walk = Walk {
            windows,
            base,
            end: base + fence.unwrap_or(segment.len()),
            spans: &mut spans,
        };
        walk.node(&linked, None, StyleFlags::NONE);
        match fence {
            Some(end) => {
                base += end;
                tail = parse(&text[base..]);
                root = &tail;
            }
            None => break,
        }
    }
    spans
}

fn pack(spans: Vec<ByteSpan>, mut to_utf16: impl FnMut(usize) -> usize) -> HighlightSpans {
    let mut packed = Vec::with_capacity(spans.len() * 3);
    for span in spans {
        let from = to_utf16(span.range.start);
        let to = to_utf16(span.range.end);
        packed.extend([
            from as u32,
            to as u32,
            u32::from(span.role as u8) | u32::from(span.flags.0) << 8,
        ]);
    }
    HighlightSpans { packed }
}

/// Окна в байтах: прижатые к тексту, отсортированные, слитые с соседями.
/// Пустой список — весь текст.
fn byte_windows(
    text: &str,
    windows: &[HighlightWindow],
    mut to_byte: impl FnMut(usize) -> usize,
) -> Vec<Range<usize>> {
    if windows.is_empty() {
        return vec![0..text.len()];
    }
    let mut ranges: Vec<Range<usize>> = windows
        .iter()
        .map(|w| to_byte(w.from.min(w.to))..to_byte(w.to.max(w.from)))
        .filter(|range| !range.is_empty())
        .collect();
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// Начало первой ошибки «незакрыто», которую спека держит в пределах абзаца:
/// скобки, `$` (у Typst — «unclosed delimiter») и строка. Незакрытые блочный
/// комментарий и raw идут до конца текста, как у Typst (FR-006), — их здесь нет.
fn first_unclosed(node: &LinkedNode) -> Option<usize> {
    // Признак «в поддереве есть ошибка» Typst хранит готовым: без него поиск
    // обходил бы всё дерево на каждое нажатие.
    if !node.get().diagnosis().errors {
        return None;
    }
    if node.kind() == SyntaxKind::Error {
        let (errors, _) = node.get().errors_and_warnings();
        let unclosed = errors.iter().any(|error| {
            error.message == "unclosed delimiter" || error.message == "unclosed string"
        });
        return unclosed.then(|| node.offset());
    }
    node.children().find_map(|child| first_unclosed(&child))
}

/// Начало строки, следующей за первой пустой строкой после `at`; `None` —
/// абзац тянется до конца текста.
fn paragraph_end(text: &str, at: usize) -> Option<usize> {
    let mut lines = text[at..].split_inclusive('\n');
    let mut offset = at + lines.next()?.len();
    for line in lines {
        offset += line.len();
        if line.trim().is_empty() && line.ends_with('\n') {
            return (offset < text.len()).then_some(offset);
        }
    }
    None
}

struct ByteSpan {
    range: Range<usize>,
    role: SyntaxRole,
    flags: StyleFlags,
}

/// Обход одного разобранного куска текста: `base` — его начало в заметке,
/// `end` — граница, дальше которой фрагменты обрезаются.
struct Walk<'w> {
    windows: &'w [Range<usize>],
    base: usize,
    end: usize,
    spans: &'w mut Vec<ByteSpan>,
}

impl Walk<'_> {
    fn intersects(&self, range: &Range<usize>) -> bool {
        // Окна отсортированы и не пересекаются: первое окно, кончающееся правее
        // начала узла, — единственный кандидат.
        let at = self.windows.partition_point(|w| w.end <= range.start);
        self.windows.get(at).is_some_and(|w| w.start < range.end)
    }

    /// `inherited` — роль контейнера для листьев без своей (`Raw`, `Ref`,
    /// термин списка); `flags` — начертание от предков.
    fn node(&mut self, node: &LinkedNode, inherited: Option<SyntaxRole>, flags: StyleFlags) {
        let local = node.range();
        let range = self.base + local.start..(self.base + local.end).min(self.end);
        if range.is_empty() || !self.intersects(&range) {
            return;
        }
        let (own, flags) = match role_of(node) {
            Some(NodeRole::Role(role)) => (Some(role), flags),
            Some(NodeRole::Style(style)) => (None, flags | style),
            None => (None, flags),
        };

        if node.get().children().len() == 0 {
            let role = own.or(inherited).or_else(|| {
                // Текст без роли, но внутри `*…*` или заголовка, тоже нужен
                // редактору: у него своё начертание.
                (flags != StyleFlags::NONE && !is_blank(node.kind())).then_some(SyntaxRole::Plain)
            });
            if let Some(role) = role {
                self.push(range, role, flags);
            }
            return;
        }

        // Роль контейнера не переходит через границу вложенного содержимого:
        // `[…]` в ссылке или формула в аргументе — уже другой режим.
        let inherited = match node.kind() {
            SyntaxKind::ContentBlock | SyntaxKind::CodeBlock | SyntaxKind::Equation => None,
            _ => own.or(inherited),
        };
        for child in node.children() {
            self.node(&child, inherited, flags);
        }
    }

    /// Соседние листья с одной ролью и начертанием — один фрагмент.
    fn push(&mut self, range: Range<usize>, role: SyntaxRole, flags: StyleFlags) {
        if let Some(last) = self.spans.last_mut() {
            if last.range.end == range.start && last.role == role && last.flags == flags {
                last.range.end = range.end;
                return;
            }
        }
        self.spans.push(ByteSpan { range, role, flags });
    }
}

fn is_blank(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Space | SyntaxKind::Parbreak)
}
