"use client";

import { useEffect, useRef } from "react";
import { EditorState } from "@codemirror/state";
import { Decoration, EditorView, keymap, type DecorationSet } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import type { LineOrigin } from "../wasm-core/typsidian_core.js";
import { originClass, originMark } from "./mergeMarks.js";

export interface MergeResultProps {
  value: string;
  /** Пометки строк от ядра; строки сверх них ещё не пересчитаны — «вручную». */
  lines: LineOrigin[];
  ariaLabel: string;
  readOnly: boolean;
  onChange: (next: string) => void;
}

function lineDecorations(state: EditorState, lines: LineOrigin[]): DecorationSet {
  const ranges = [];
  for (let number = 1; number <= state.doc.lines; number += 1) {
    const line = state.doc.line(number);
    // Пустая строка после завершающего `\n` — не строка текста: у ядра её нет.
    const trailing = number === state.doc.lines && line.length === 0;
    const origin: LineOrigin = lines[number - 1] ?? (trailing ? "Same" : "Manual");
    ranges.push(
      Decoration.line({
        class: `ts-merge-line ${originClass(origin)}`,
        attributes: { "data-mark": originMark(origin) },
      }).range(line.from),
    );
  }
  return Decoration.set(ranges);
}

/**
 * Поле результата слияния (фича 040, research §5): текст правится прямо здесь,
 * а у каждой строки — фон и маркер стороны, откуда она взята. Пометки считает
 * ядро (`mergeAnnotate`); поле их только рисует.
 */
export function MergeResult({
  value,
  lines,
  ariaLabel,
  readOnly,
  onChange,
}: MergeResultProps): React.JSX.Element {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  // Свежие значения для расширений, созданных один раз.
  const latest = useRef({ lines, ariaLabel, readOnly, onChange });
  latest.current = { lines, ariaLabel, readOnly, onChange };

  useEffect(() => {
    const host = hostRef.current;
    if (host === null) return undefined;
    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of(() => ({ "aria-label": latest.current.ariaLabel })),
          EditorState.changeFilter.of(() => !latest.current.readOnly),
          EditorView.decorations.of((current) =>
            lineDecorations(current.state, latest.current.lines),
          ),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) latest.current.onChange(update.state.doc.toString());
          }),
        ],
      }),
    });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // Экземпляр один на жизнь поля: текст и пометки обновляются ниже.
  }, []);

  // Текст сменился не набором: пересобран из выбора сторон.
  useEffect(() => {
    const view = viewRef.current;
    if (view === null || view.state.doc.toString() === value) return;
    const wasReadOnly = latest.current.readOnly;
    latest.current.readOnly = false;
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } });
    latest.current.readOnly = wasReadOnly;
  }, [value]);

  // Пришли новые пометки — перерисовать.
  useEffect(() => {
    viewRef.current?.dispatch({});
  }, [lines]);

  return (
    <div
      ref={hostRef}
      className="ts-merge-result"
      data-readonly={readOnly ? "true" : undefined}
    />
  );
}
