import { StateEffect, StateField, type Extension } from "@codemirror/state";
import { Decoration, EditorView, hoverTooltip, type DecorationSet } from "@codemirror/view";

// Отметки в тексте редактора (фича 017 заводит, 018 пользуется;
// contracts/frontend-sources.md § Отметки).
//
// Отметки хранятся по поставщикам: `setMarks(view, "sources", …)` заменяет
// только свой набор. Диапазоны переносятся через правки до следующего
// `setMarks`; что вышло за пределы текста, отбрасывается, а не роняет
// редактор. Причины приходят уже переведёнными — здесь только подача.

export interface EditorMark {
  /** Смещения в кодовых единицах UTF-16, как у `SourcePlace`. */
  from: number;
  to: number;
  tone: "error" | "warning";
  message: string;
}

interface Marked {
  mark: EditorMark;
  from: number;
  to: number;
}

const setProviderMarks = StateEffect.define<{ provider: string; marks: EditorMark[] }>();

const marksField = StateField.define<ReadonlyMap<string, Marked[]>>({
  create: () => new Map(),
  update(value, transaction) {
    let next = value;
    if (transaction.docChanged && value.size > 0) {
      const mapped = new Map<string, Marked[]>();
      for (const [provider, marks] of value) {
        mapped.set(
          provider,
          marks
            .map((item) => ({
              ...item,
              from: transaction.changes.mapPos(item.from, 1),
              to: transaction.changes.mapPos(item.to, -1),
            }))
            .filter((item) => item.to > item.from),
        );
      }
      next = mapped;
    }
    for (const effect of transaction.effects) {
      if (!effect.is(setProviderMarks)) continue;
      const length = transaction.state.doc.length;
      const copy = new Map(next);
      copy.set(
        effect.value.provider,
        effect.value.marks
          .filter((mark) => mark.from >= 0 && mark.to <= length && mark.to > mark.from)
          .map((mark) => ({ mark, from: mark.from, to: mark.to })),
      );
      next = copy;
    }
    return next;
  },
  provide: (field) =>
    EditorView.decorations.from(field, (value): DecorationSet => {
      const ranges = [...value.values()]
        .flat()
        .sort((left, right) => left.from - right.from || left.to - right.to)
        .map((item) =>
          Decoration.mark({
            class: item.mark.tone === "error" ? "ts-mark--error" : "ts-mark--warning",
          }).range(item.from, item.to),
        );
      return Decoration.set(ranges, true);
    }),
});

/** Подсказка по наведению — причина отметки под указателем. */
const marksTooltip = hoverTooltip((view, pos) => {
  const found = [...view.state.field(marksField).values()]
    .flat()
    .filter((item) => item.from <= pos && pos <= item.to);
  if (found.length === 0) return null;
  const first = found[0]!;
  return {
    pos: first.from,
    end: first.to,
    above: true,
    create: () => {
      const dom = document.createElement("div");
      dom.className = "ts-mark-tip";
      dom.textContent = found.map((item) => item.mark.message).join("\n");
      return { dom };
    },
  };
});

/** Поле отметок и подсказка — подключается каждому редактору заметки. */
export const editorMarks: Extension = [marksField, marksTooltip];

/** Заменяет набор отметок одного поставщика. */
export function setMarks(view: EditorView, provider: string, marks: EditorMark[]): void {
  if (view.state.field(marksField, false) === undefined) return;
  view.dispatch({ effects: setProviderMarks.of({ provider, marks }) });
}
