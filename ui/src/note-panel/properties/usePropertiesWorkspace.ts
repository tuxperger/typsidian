"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { EditorView } from "@codemirror/view";
import type { PropertyEdit } from "../../wasm-core/typsidian_core.js";
import type { VaultClient } from "../../vault-client.js";
import type { PropertiesEditResult } from "./PropertiesMode.js";

// Связка свойств с рабочей областью (фича 020): применение правки записи к
// редактору, пересборка заметки с блоками выборки и пересчёт индекса при
// открытии vault. Правил здесь нет: план правки, отпечаток входов и сам
// пересчёт — ядро.

export interface PropertiesWorkspaceInput {
  client: VaultClient | null;
  /** Открытая заметка (не доска); `null` — нечего собирать. */
  notePath: string | null;
  /** Текст заметки в редакторе. */
  text: string;
  /** Растёт на события vault и удачные записи (то же, что у колонки связей). */
  revision: number;
  /** Живой редактор активной заметки. */
  getEditorView: (() => EditorView | null) | null;
  /** Входы блоков выборки изменились: пересобрать предпросмотр. */
  onRebuild: () => void;
}

export interface ReindexView {
  done: number;
  total: number;
}

export interface PropertiesWorkspace {
  applyEdit: (change: PropertyEdit) => Promise<PropertiesEditResult>;
  /** Идущий пересчёт индекса; `null` — не идёт. */
  reindex: ReindexView | null;
  cancelReindex: () => void;
}

export function usePropertiesWorkspace({
  client,
  notePath,
  text,
  revision,
  getEditorView,
  onRebuild,
}: PropertiesWorkspaceInput): PropertiesWorkspace {
  const textRef = useRef(text);
  textRef.current = text;
  const rebuildRef = useRef(onRebuild);
  rebuildRef.current = onRebuild;

  // ── Правка записи ────────────────────────────────────────────────────
  const applyEdit = useCallback(
    async (change: PropertyEdit): Promise<PropertiesEditResult> => {
      const view = getEditorView?.() ?? null;
      if (client === null || view === null) return { kind: "refused", problem: null };
      const outcome = await client.properties.edit(view.state.doc.toString(), change);
      if (outcome.kind === "Refused") {
        return {
          kind: "refused",
          problem: outcome.error.kind === "Properties" ? outcome.error.problem : null,
        };
      }
      if (outcome.edits.length > 0) {
        // Все замены - одной транзакцией, одним шагом отмены (как у параметров 008).
        view.dispatch({
          changes: outcome.edits.map((edit) => ({
            from: edit.from,
            to: edit.to,
            insert: edit.insert,
          })),
          userEvent: "input",
        });
      }
      return { kind: "applied" };
    },
    [client, getEditorView],
  );

  // ── Отпечаток входов блоков (SC-002) ─────────────────────────────────
  const digest = useRef<{ path: string; value: string | undefined } | null>(null);
  useEffect(() => {
    if (client === null || notePath === null) {
      digest.current = null;
      return undefined;
    }
    let stale = false;
    void client.properties
      .selectDigest(textRef.current)
      .then((value) => {
        if (stale) return;
        const before = digest.current;
        digest.current = { path: notePath, value };
        // Пересборка нужна, только если отпечаток той же заметки сменился.
        if (before !== null && before.path === notePath && before.value !== value) {
          rebuildRef.current();
        }
      })
      .catch(() => undefined);
    return () => {
      stale = true;
    };
  }, [client, notePath, revision]);

  // ── Пересчёт индекса при открытии vault ──────────────────────────────
  const [reindex, setReindex] = useState<ReindexView | null>(null);
  const abort = useRef<AbortController | null>(null);
  useEffect(() => {
    if (client === null) return undefined;
    const controller = new AbortController();
    abort.current = controller;
    void (async () => {
      try {
        if (await client.properties.current()) return;
        setReindex({ done: 0, total: 0 });
        await client.properties.reindex({
          signal: controller.signal,
          onProgress: (done, total) => setReindex({ done, total }),
        });
      } catch {
        // Пересчёт - фон: отказ не мешает работе, индекс догонится при открытии.
      } finally {
        setReindex(null);
      }
    })();
    return () => controller.abort();
  }, [client]);
  const cancelReindex = useCallback(() => abort.current?.abort(), []);

  return { applyEdit, reindex, cancelReindex };
}
