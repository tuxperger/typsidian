"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  OpenText,
  ReplaceNote,
  ReplacePreview,
  ReplaceQuery,
  UndoRecord,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import type { OpenNoteEditors } from "../note-panel/links/openNoteEditors.js";
import type { Phrase } from "../i18n/index.js";

// Замена по vault (фича 027). Здесь только ритм: когда спросить предпросмотр,
// какие места отмечены, в каком порядке и какими порциями звать ядро. Что
// совпало, на что заменить, свеж ли текст и куда писать — решает ядро
// (`search::replace`, принцип I).

/** Пауза после последнего нажатия перед предпросмотром — как у поиска. */
export const REPLACE_DEBOUNCE_MS = 200;

/** Отмеченные номера совпадений по пути заметки. */
export type Selection = ReadonlyMap<string, ReadonlySet<number>>;

/** Итог применения (макет R2). */
export interface ReplaceReport {
  matches: number;
  notes: string[];
  stale: string[];
  unsaved: string[];
  failed: string[];
  readOnly: boolean;
  /** Остановлено пользователем: сколько заметок изменено из скольких. */
  stopped: { done: number; total: number } | null;
}

/** Итог отмены. */
export interface UndoReport {
  restored: number;
  changed: string[];
}

export type ReplacePhase =
  | { kind: "idle" }
  | { kind: "running"; done: number; total: number }
  | { kind: "finished"; report: ReplaceReport }
  | { kind: "undone"; report: UndoReport };

export interface ReplaceState {
  preview: ReplacePreview | null;
  /** Отказ предпросмотра словами (неверное выражение). */
  error: Phrase | null;
  /** Совпадений больше предела — сколько (FR-008). */
  tooMany: number | null;
  busy: boolean;
  selection: Selection;
  selectedCount: number;
  selectedNotes: number;
  phase: ReplacePhase;
  toggleMatch: (path: string, index: number) => void;
  toggleNote: (path: string) => void;
  apply: () => Promise<void>;
  stop: () => void;
  undo: () => Promise<void>;
  canUndo: boolean;
  refresh: () => void;
}

export interface UseReplaceInput {
  client: VaultClient;
  editors: OpenNoteEditors;
  openTexts: () => readonly OpenText[];
  query: ReplaceQuery;
  /** Режим замены включён и строка поиска не пустая. */
  enabled: boolean;
}

function allOf(preview: ReplacePreview): Map<string, Set<number>> {
  return new Map(
    preview.notes.map((note) => [note.path, new Set(note.matches.map((match) => match.index))]),
  );
}

/** Отказ ядра: неверное выражение, слишком много совпадений или чужая ошибка. */
function classify(cause: unknown): { tooMany: number } | { error: Phrase } {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const kind = (cause as { kind: unknown }).kind;
    if (kind === "TooMany" && "count" in cause) {
      return { tooMany: Number((cause as { count: unknown }).count) };
    }
    if (kind === "BadPattern" && "message" in cause) {
      const detail = String((cause as { message: unknown }).message);
      return { error: (t) => t("search.badPattern", { detail }) };
    }
  }
  return { error: () => String(cause) };
}

/** Даёт интерфейсу перерисоваться между заметками (SC-004). */
const yieldToUi = (): Promise<void> => new Promise((resolve) => window.setTimeout(resolve, 0));

export function useReplace({
  client,
  editors,
  openTexts,
  query,
  enabled,
}: UseReplaceInput): ReplaceState {
  const [preview, setPreview] = useState<ReplacePreview | null>(null);
  const [error, setError] = useState<Phrase | null>(null);
  const [tooMany, setTooMany] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [selection, setSelection] = useState<Map<string, Set<number>>>(new Map());
  const [phase, setPhase] = useState<ReplacePhase>({ kind: "idle" });
  // Пакет последней замены (research.md §4): живёт до закрытия поиска или
  // следующего пакета.
  const [batch, setBatch] = useState<UndoRecord[]>([]);
  const [tick, setTick] = useState(0);
  const revision = useRef(0);
  const stopRef = useRef(false);
  const openTextsRef = useRef(openTexts);
  openTextsRef.current = openTexts;

  // Предпросмотр зависит от содержимого запроса, а не от объекта.
  const key = JSON.stringify(query);
  useEffect(() => {
    const mine = ++revision.current;
    if (!enabled) {
      setPreview(null);
      setSelection(new Map());
      setError(null);
      setTooMany(null);
      setBusy(false);
      return;
    }
    setBusy(true);
    const timer = window.setTimeout(() => {
      void client
        .replacePreview(JSON.parse(key) as ReplaceQuery, [...openTextsRef.current()])
        .then((found) => {
          if (mine !== revision.current) return;
          setPreview(found);
          setSelection(allOf(found));
          setError(null);
          setTooMany(null);
        })
        .catch((cause: unknown) => {
          if (mine !== revision.current) return;
          // Отметки прежней выдачи не переживают отказ: замену нечем применять.
          setPreview(null);
          setSelection(new Map());
          const outcome = classify(cause);
          if ("tooMany" in outcome) {
            setTooMany(outcome.tooMany);
            setError(null);
          } else {
            setError(() => outcome.error);
            setTooMany(null);
          }
        })
        .finally(() => {
          if (mine === revision.current) setBusy(false);
        });
    }, REPLACE_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [client, key, enabled, tick]);

  const toggleMatch = useCallback((path: string, index: number) => {
    setSelection((current) => {
      const next = new Map(current);
      const marks = new Set(next.get(path) ?? []);
      if (marks.has(index)) marks.delete(index);
      else marks.add(index);
      next.set(path, marks);
      return next;
    });
  }, []);

  const toggleNote = useCallback(
    (path: string) => {
      const note = preview?.notes.find((entry) => entry.path === path);
      if (!note) return;
      setSelection((current) => {
        const next = new Map(current);
        const marks = next.get(path);
        const all = marks !== undefined && marks.size === note.matches.length;
        next.set(path, all ? new Set() : new Set(note.matches.map((match) => match.index)));
        return next;
      });
    },
    [preview],
  );

  const { selectedCount, selectedNotes } = useMemo(() => {
    let count = 0;
    let notes = 0;
    for (const marks of selection.values()) {
      count += marks.size;
      if (marks.size > 0) notes += 1;
    }
    return { selectedCount: count, selectedNotes: notes };
  }, [selection]);

  const apply = useCallback(async () => {
    if (preview === null || phase.kind === "running") return;
    const chosen: { note: ReplaceNote; selected: number[] }[] = preview.notes
      .map((note) => ({ note, selected: [...(selection.get(note.path) ?? [])] }))
      .filter((entry) => entry.selected.length > 0);
    if (chosen.length === 0) return;

    stopRef.current = false;
    const report: ReplaceReport = {
      matches: 0,
      notes: [],
      stale: [],
      unsaved: [],
      failed: [],
      readOnly: false,
      stopped: null,
    };
    const undo: UndoRecord[] = [];
    setPhase({ kind: "running", done: 0, total: chosen.length });

    for (const [position, { note, selected }] of chosen.entries()) {
      if (stopRef.current) {
        report.stopped = { done: report.notes.length, total: chosen.length };
        break;
      }
      const editorText = editors.textOf(note.path);
      const outcome = await client.replaceInNote(
        { query, path: note.path, digest: note.digest, selected },
        editorText,
      );
      switch (outcome.kind) {
        case "Written":
          undo.push(outcome.undo);
          report.notes.push(note.name);
          report.matches += selected.length;
          break;
        case "EditorEdit":
          if (editors.applyEdit(note.path, outcome.edit).ok) {
            undo.push(outcome.undo);
            report.notes.push(note.name);
            report.matches += selected.length;
          } else {
            report.stale.push(note.name);
          }
          break;
        case "Stale":
          report.stale.push(note.name);
          break;
        case "UnsavedElsewhere":
          report.unsaved.push(note.name);
          break;
        case "ReadOnly":
          report.readOnly = true;
          break;
        case "Failed":
          report.failed.push(note.name);
          break;
        case "NothingSelected":
          break;
        default: {
          const exhaustive: never = outcome;
          return exhaustive;
        }
      }
      if (report.readOnly) break;
      setPhase({ kind: "running", done: position + 1, total: chosen.length });
      await yieldToUi();
    }

    setBatch(undo);
    setPhase({ kind: "finished", report });
    // Выдача после замены — по новым текстам.
    setTick((value) => value + 1);
  }, [client, editors, phase.kind, preview, query, selection]);

  const stop = useCallback(() => {
    stopRef.current = true;
  }, []);

  const undo = useCallback(async () => {
    if (batch.length === 0) return;
    const report: UndoReport = { restored: 0, changed: [] };
    const nameOf = (path: string): string => path.split("/").pop()?.replace(/\.typ$/, "") ?? path;
    for (const record of [...batch].reverse()) {
      const outcome = await client.replaceUndo(record, editors.textOf(record.path));
      switch (outcome.kind) {
        case "Restored":
          report.restored += 1;
          break;
        case "EditorEdit":
          if (editors.applyEdit(record.path, outcome.edit).ok) report.restored += 1;
          else report.changed.push(nameOf(record.path));
          break;
        case "ChangedSince":
        case "Missing":
        case "UnsavedElsewhere":
        case "ReadOnly":
        case "Failed":
          report.changed.push(nameOf(record.path));
          break;
        default: {
          const exhaustive: never = outcome;
          return exhaustive;
        }
      }
      await yieldToUi();
    }
    setBatch([]);
    setPhase({ kind: "undone", report });
    setTick((value) => value + 1);
  }, [batch, client, editors]);

  const refresh = useCallback(() => setTick((value) => value + 1), []);

  return {
    preview,
    error,
    tooMany,
    busy,
    selection,
    selectedCount,
    selectedNotes,
    phase,
    toggleMatch,
    toggleNote,
    apply,
    stop,
    undo,
    canUndo: batch.length > 0,
    refresh,
  };
}
