"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { EditorView } from "@codemirror/view";
import type { VaultClient } from "../vault-client.js";
import type {
  CompletionItem,
  CompletionList,
  LinkedIndex,
  SourceKey,
} from "../wasm-core/typsidian_core.js";
import type { CaretAnchor } from "../editor/useMenuPosition.js";
import type { CompletionKeysPort } from "./completionKeys.js";

// Сеанс списка дополнения (фича 018, contracts/frontend-completion.md
// § useCompletion). Правил здесь нет: где каретка и что подходит, отвечает
// ядро; хук решает, когда спросить, отбрасывает опоздавшие ответы и
// применяет выбор одной транзакцией.

export interface CompletionSession {
  list: CompletionList;
  /** Строки всех групп подряд — по ним ходят ↑↓. */
  items: CompletionItem[];
  active: number;
  anchor: CaretAnchor | null;
}

export interface CompletionState {
  session: CompletionSession | null;
  /** Набран `@` или `[[`: спросить, если включён показ при наборе. */
  onTrigger(): void;
  /** Ручной вызов (FR-009): работает и при выключенном показе. */
  trigger(): void;
  /** Текст или каретка изменились: открытый список спрашивает заново. */
  onCaretMove(): void;
  pick(index: number): void;
  setActive(index: number): void;
  close(): void;
  keys: CompletionKeysPort;
}

export interface CompletionInput {
  client: VaultClient | null;
  notePath: string;
  view: () => EditorView | null;
  caretCoords: () => CaretAnchor | null;
  linked: LinkedIndex | null;
  /** Ключи источников 017 под набранную часть; `null` — библиографии нет. */
  sourcesFor: ((query: string) => Promise<SourceKey[]>) | null;
  auto: boolean;
  enabled: boolean;
}

export function useCompletion(input: CompletionInput): CompletionState {
  const [session, setSession] = useState<CompletionSession | null>(null);
  const latest = useRef(input);
  latest.current = input;
  const asked = useRef(0);
  const open = useRef(false);
  const manual = useRef(false);
  const sessionRef = useRef<CompletionSession | null>(null);
  sessionRef.current = session;

  const close = useCallback(() => {
    asked.current += 1;
    open.current = false;
    manual.current = false;
    setSession(null);
  }, []);

  const ask = useCallback(
    async (byCommand: boolean) => {
      const { client, notePath, view, caretCoords, linked, sourcesFor, enabled } = latest.current;
      const editor = view();
      if (client === null || editor === null || !enabled) return;
      const text = editor.state.doc.toString();
      const offset = editor.state.selection.main.head;
      const ticket = ++asked.current;
      const request = {
        note_path: notePath,
        text,
        offset,
        manual: byCommand,
        linked: linked ?? { note_path: notePath, files: [], unreadable: [] },
        sources: [] as SourceKey[],
      };
      try {
        let list = await client.completions(request);
        if (ticket !== asked.current) return;
        // Ключи источников — только под набранную часть: весь перечень на
        // каждое нажатие гонять незачем.
        if (list.target.kind === "Label" && sourcesFor !== null) {
          const sources = await sourcesFor(list.query);
          if (ticket !== asked.current) return;
          if (sources.length > 0) {
            list = await client.completions({ ...request, sources });
            if (ticket !== asked.current) return;
          }
        }
        if (list.target.kind === "None" || list.groups.length === 0) {
          open.current = false;
          setSession(null);
          return;
        }
        const items = list.groups.flatMap((group) => group.items);
        open.current = true;
        setSession((previous) => ({
          list,
          items,
          // Выбор держится на той же строке, пока она есть.
          active: Math.max(
            0,
            previous === null
              ? 0
              : items.findIndex((item) => item.label === previous.items[previous.active]?.label),
          ),
          anchor: caretCoords(),
        }));
      } catch {
        // Ядро отказало — список просто не показывается.
      }
    },
    [],
  );

  const onTrigger = useCallback(() => {
    if (!latest.current.auto) return;
    void ask(false);
  }, [ask]);

  const trigger = useCallback(() => {
    manual.current = true;
    void ask(true);
  }, [ask]);

  const onCaretMove = useCallback(() => {
    if (open.current) void ask(manual.current);
  }, [ask]);

  const pick = useCallback(
    (index: number) => {
      const current = sessionRef.current;
      const editor = latest.current.view();
      const item = current?.items[index];
      if (current === null || editor === null || item === undefined) return;
      const { from, to } = current.list;
      editor.dispatch({
        changes: { from, to, insert: item.insert },
        selection: { anchor: from + item.insert.length },
        userEvent: "input.complete",
        scrollIntoView: true,
      });
      close();
      editor.focus();
    },
    [close],
  );

  const setActive = useCallback((index: number) => {
    setSession((current) => (current === null ? null : { ...current, active: index }));
  }, []);

  // Смена заметки закрывает список.
  useEffect(() => close, [input.notePath, close]);

  const keys = useMemo<CompletionKeysPort>(
    () => ({
      isOpen: () => open.current && sessionRef.current !== null,
      move: (delta) =>
        setSession((current) => {
          if (current === null || current.items.length === 0) return current;
          const count = current.items.length;
          return { ...current, active: (current.active + delta + count) % count };
        }),
      accept: () => {
        const current = sessionRef.current;
        if (current !== null) pick(current.active);
      },
      close,
    }),
    [pick, close],
  );

  return { session, onTrigger, trigger, onCaretMove, pick, setActive, close, keys };
}
