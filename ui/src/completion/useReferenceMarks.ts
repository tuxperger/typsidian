"use client";

import { useEffect, useRef, useState } from "react";
import type { EditorView } from "@codemirror/view";
import type { VaultClient } from "../vault-client.js";
import type {
  BuildEvidence,
  LinkedIndex,
  ReferenceMark,
} from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import { setMarks } from "../editor/marks.js";
import { markText } from "./describe.js";

/** Пауза после правки перед пересчётом (contracts/frontend-completion.md). */
const MARKS_DELAY_MS = 300;
/** Сколько ждать сборку новой правки, прежде чем ставить новые отметки. */
const HOLD_MS = 800;

export interface ReferenceMarksInput {
  client: VaultClient | null;
  view: () => EditorView | null;
  notePath: string;
  text: string;
  linked: LinkedIndex | null;
  /** Последняя удачная сборка этой заметки. */
  evidence: BuildEvidence | null;
  /** Сборка текущего текста ещё не вернулась. */
  building: boolean;
  sourceKeys: readonly string[];
  /** Диапазоны, уже отмеченные 017. */
  claimed: readonly [number, number][];
  /** Растёт по событию vault: заметка создана или удалена. */
  vaultRevision: number;
  enabled: boolean;
}

/**
 * Отметки ссылок в никуда и повторных меток (фича 018, FR-006, FR-007).
 * Правило — в ядре; здесь ритм вопросов и удержание: новая отметка «метки
 * нет» ждёт сборку правки до 800 мс — ссылка на метку из кода не мигает.
 */
export function useReferenceMarks(input: ReferenceMarksInput): ReferenceMark[] {
  const { t } = useI18n();
  const [marks, setMarksState] = useState<ReferenceMark[]>([]);
  const previous = useRef<string[]>([]);
  const asked = useRef(0);
  const [holdOver, setHoldOver] = useState(true);
  const { client, view, notePath, text, linked, evidence, building, sourceKeys, claimed } = input;

  // Правка начинает удержание заново; через 800 мс оно снимается само.
  useEffect(() => {
    setHoldOver(false);
    const timer = window.setTimeout(() => setHoldOver(true), HOLD_MS);
    return () => window.clearTimeout(timer);
  }, [text]);

  useEffect(() => {
    previous.current = [];
    setMarksState([]);
  }, [notePath]);

  useEffect(() => {
    if (!input.enabled || client === null) {
      const editor = view();
      if (editor !== null) setMarks(editor, "references", []);
      return;
    }
    const held = building && !holdOver;
    const ticket = ++asked.current;
    const timer = window.setTimeout(() => {
      void client
        .referenceMarks({
          note_path: notePath,
          text,
          linked: linked ?? { note_path: notePath, files: [], unreadable: [] },
          ...(evidence === null ? {} : { evidence }),
          sources: [...sourceKeys],
          claimed: claimed.map(([from, to]) => [from, to]),
          held,
          previously_missing: previous.current,
        })
        .then((found) => {
          if (ticket !== asked.current) return;
          const editor = view();
          // Текст ушёл вперёд — места были бы мимо.
          if (editor === null || editor.state.doc.toString() !== text) return;
          previous.current = found.flatMap((mark) =>
            mark.kind.kind === "MissingLabel" ? [mark.kind.name] : [],
          );
          setMarks(
            editor,
            "references",
            found.map((mark) => {
              const { title, detail } = markText(t, mark.kind);
              return { from: mark.from, to: mark.to, tone: "note", message: `${title}\n${detail}` };
            }),
          );
          setMarksState(found);
        })
        .catch(() => {
          // Ядро отказало — прежние отметки остаются до следующего ответа.
        });
    }, MARKS_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [
    client,
    view,
    notePath,
    text,
    linked,
    evidence,
    building,
    holdOver,
    sourceKeys,
    claimed,
    input.vaultRevision,
    input.enabled,
    t,
  ]);

  return marks;
}
