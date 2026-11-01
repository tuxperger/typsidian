import { RangeSet, type EditorState, type Extension } from "@codemirror/state";
import { Decoration, EditorView, GutterMarker, gutterLineClass } from "@codemirror/view";
import type { DiagnosticLocation } from "../wasm-core/typsidian_core.js";

// Место ошибки сборки в тексте (фича 044, артборд 5): фрагмент подчёркнут
// волной, номер строки — цветом акцента. Что и где сломано, решает ядро
// (диагностика сборки 005); здесь только показ уже известного места.

const errorMark = Decoration.mark({ class: "ts-build-error" });

class ErrorLineMarker extends GutterMarker {
  override elementClass = "ts-build-error-line";
}

const errorLine = new ErrorLineMarker();

/** Длина символа в UTF-8 — ядро меряет диапазон замечания в байтах. */
function utf8Length(code: number): number {
  if (code < 0x80) return 1;
  if (code < 0x800) return 2;
  if (code < 0x10000) return 3;
  return 4;
}

interface Span {
  from: number;
  to: number;
  lineFrom: number;
}

function spansOf(state: EditorState, places: readonly DiagnosticLocation[]): Span[] {
  const doc = state.doc;
  const spans: Span[] = [];
  for (const place of places) {
    if (place.line < 1 || place.line > doc.lines) continue;
    const line = doc.line(place.line);
    const from = Math.min(line.to, line.from + Math.max(0, place.column - 1));
    // Конец — по байтовой длине диапазона, но не дальше строки: многострочное
    // замечание подчёркивается до конца первой строки.
    let bytes = Math.max(1, place.byte_end - place.byte_start);
    let to = from;
    while (to < line.to && bytes > 0) {
      const code = doc.sliceString(to, Math.min(line.to, to + 2)).codePointAt(0) ?? 0;
      bytes -= utf8Length(code);
      to += code > 0xffff ? 2 : 1;
    }
    spans.push({ from, to: Math.max(to, Math.min(line.to, from + 1)), lineFrom: line.from });
  }
  return spans.sort((a, b) => a.from - b.from || a.to - b.to);
}

/** Расширение редактора для мест ошибок заметки; пустой список — ничего. */
export function buildErrorMarks(places: readonly DiagnosticLocation[]): Extension {
  if (places.length === 0) return [];
  return [
    EditorView.decorations.compute(["doc"], (state) =>
      Decoration.set(
        spansOf(state, places)
          .filter((span) => span.to > span.from)
          .map((span) => errorMark.range(span.from, span.to)),
        true,
      ),
    ),
    gutterLineClass.compute(["doc"], (state) =>
      RangeSet.of(
        spansOf(state, places).map((span) => errorLine.range(span.lineFrom)),
        true,
      ),
    ),
  ];
}
