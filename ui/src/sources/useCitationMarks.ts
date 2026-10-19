"use client";

import { useEffect, useRef, useState } from "react";
import type { EditorView } from "@codemirror/view";
import type { VaultClient } from "../vault-client.js";
import type { NoteCheck } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import { setMarks } from "../editor/marks.js";
import { citationMarks } from "./describe.js";

/** Пауза после набора перед пересчётом отметок (contracts/frontend-sources.md). */
const CHECK_DELAY_MS = 300;

/**
 * Отметки битых цитат в редакторе заметки (FR-006, SC-003): живой текст →
 * пауза → `checkNote` → `setMarks(view, "sources", …)`. Смена библиотеки
 * (`revision`) пересчитывает сразу. Ответ на устаревший текст отбрасывается.
 */
export function useCitationMarks(
  client: VaultClient | null,
  view: () => EditorView | null,
  text: string,
  revision: number,
  enabled: boolean,
): NoteCheck | null {
  const { t } = useI18n();
  const [check, setCheck] = useState<NoteCheck | null>(null);
  const lastRevision = useRef(revision);
  const asked = useRef(0);

  useEffect(() => {
    if (!enabled || client === null) {
      const editor = view();
      if (editor !== null) setMarks(editor, "sources", []);
      setCheck(null);
      return;
    }
    const immediate = lastRevision.current !== revision;
    lastRevision.current = revision;
    const ticket = ++asked.current;
    const timer = window.setTimeout(
      () => {
        void client.sources.checkNote(text, []).then((result) => {
          if (ticket !== asked.current) return;
          const editor = view();
          // Текст ушёл вперёд — ответ про прежний, отметки встали бы мимо.
          if (editor === null || editor.state.doc.toString() !== text) return;
          setMarks(editor, "sources", citationMarks(t, result.marks));
          setCheck(result);
        });
      },
      immediate ? 0 : CHECK_DELAY_MS,
    );
    return () => window.clearTimeout(timer);
  }, [client, view, text, revision, enabled, t]);

  return check;
}
