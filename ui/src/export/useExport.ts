"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type {
  BatchState,
  BatchView,
  ExportReport,
  VaultOverrides,
} from "../wasm-core/typsidian_core.js";
import type { PreviewClient } from "../preview-client.js";
import { NOTHING_OVERRIDDEN } from "../workspace/useShortcuts.js";
import { writtenBytes } from "./describe.js";

// Экспорт в PDF со стороны интерфейса (фича 009, contracts/frontend-export.md).
//
// Живёт в `Workspace`, а не в Центре экспорта: закрытие Центра не прерывает
// пакет (FR-025). Правил здесь нет — порядок, пропуск, отмена, «остановить
// при ошибке» и оценка времени считает ядро (принцип I). Хук только крутит
// цикл: взять следующую заметку → экспортировать → записать итог → уступить
// цикл событий.

/** Сколько последних файлов помнит Центр экспорта (FR-007). */
const RECENT_LIMIT = 10;

/** Итог экспорта одной заметки командой — для уведомления. */
export interface SingleExport {
  report: ExportReport;
  elapsedMs: number;
}

/** Записанный файл — строка списка «недавние». */
export interface RecentExport {
  notePath: string;
  pdfPath: string;
  bytes: number;
  elapsedMs: number;
}

/** Открытая в редакторе заметка и её текст прямо сейчас. */
export interface OpenNoteText {
  path: string | null;
  text: string;
}

export interface ExportControl {
  /** Экспорт одной заметки идёт. */
  busy: boolean;
  /** Последний итог экспорта одной заметки, пока уведомление не закрыто. */
  last: SingleExport | null;
  dismissLast: () => void;
  exportNote: (notePath: string, text: string | null) => Promise<void>;
  recent: RecentExport[];

  /** Текущий или последний пакет; `null` — пакета не было. */
  batch: BatchView | null;
  /** Пакет ещё что-то делает. */
  batchRunning: boolean;
  startBatch: (paths: string[], stopOnError: boolean) => void;
  skip: (notePath: string) => void;
  cancel: () => void;
  clearBatch: () => void;
}

export function useExport(
  client: PreviewClient | null,
  overrides: VaultOverrides | null,
  openNote: OpenNoteText,
): ExportControl {
  const settings = overrides ?? NOTHING_OVERRIDDEN;
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  // Текст открытой заметки берётся в момент, когда до неё дошла очередь, а не
  // в момент запуска пакета (FR-022) — поэтому ссылка, а не значение.
  const openNoteRef = useRef(openNote);
  openNoteRef.current = openNote;

  const [busy, setBusy] = useState(false);
  const [last, setLast] = useState<SingleExport | null>(null);
  const [recent, setRecent] = useState<RecentExport[]>([]);
  const [batch, setBatch] = useState<BatchView | null>(null);

  const remember = useCallback((report: ExportReport, elapsedMs: number) => {
    const bytes = writtenBytes(report);
    if (bytes === null) return;
    setRecent((list) =>
      [
        { notePath: report.note_path, pdfPath: report.pdf_path, bytes, elapsedMs },
        ...list.filter((entry) => entry.pdfPath !== report.pdf_path),
      ].slice(0, RECENT_LIMIT),
    );
  }, []);

  const exportNote = useCallback(
    async (notePath: string, text: string | null) => {
      if (client === null) return;
      setBusy(true);
      const started = performance.now();
      try {
        const report = await client.exportPdf(notePath, text, settingsRef.current);
        const elapsedMs = performance.now() - started;
        setLast({ report, elapsedMs });
        remember(report, elapsedMs);
      } catch (cause) {
        setLast({ report: thrown(notePath, cause), elapsedMs: 0 });
      } finally {
        setBusy(false);
      }
    },
    [client, remember],
  );

  // ── Пакет ────────────────────────────────────────────────────────────────

  // Значение пакета и цепочка его изменений. Все переходы — отмена, пропуск,
  // выдача следующей заметки, запись итога — идут строго по очереди: иначе
  // отмена, пришедшая во время `record`, была бы затёрта его результатом.
  const stateRef = useRef<BatchState | null>(null);
  const chainRef = useRef<Promise<unknown>>(Promise.resolve());
  // Поколение: смена vault или размонтирование останавливает цикл прежнего.
  const generationRef = useRef(0);

  const mutate = useCallback(
    <T,>(change: (state: BatchState) => Promise<{ state: BatchState; value: T }>) => {
      const run = chainRef.current.then(async () => {
        const current = stateRef.current;
        if (current === null || client === null) return null;
        const { state, value } = await change(current);
        stateRef.current = state;
        setBatch(await client.exportBatch.view(state));
        return value;
      });
      chainRef.current = run.catch(() => undefined);
      return run;
    },
    [client],
  );

  const running = batch !== null && !batch.is_over;

  const startBatch = useCallback(
    (paths: string[], stopOnError: boolean) => {
      if (client === null || running) return; // FR-024: один пакет за раз
      const generation = generationRef.current;
      const ops = client.exportBatch;

      void (async () => {
        const initial = await ops.start(paths, stopOnError, settingsRef.current);
        if (generationRef.current !== generation) return;
        stateRef.current = initial;
        setBatch(await ops.view(initial));

        while (generationRef.current === generation) {
          const next = await mutate(async (state) => {
            if (generationRef.current !== generation) return { state, value: null };
            const step = await ops.next(state);
            return { state: step.state, value: step.next ?? null };
          });
          if (next === null || next === undefined) break;

          const current = openNoteRef.current;
          const text = current.path === next ? current.text : null;
          const started = performance.now();
          let report: ExportReport;
          try {
            report = await client.exportPdf(next, text, settingsRef.current);
          } catch (cause) {
            report = thrown(next, cause);
          }
          const elapsedMs = performance.now() - started;
          remember(report, elapsedMs);
          await mutate(async (state) => ({
            // Итог прежнего vault в пакет нового не попадает.
            state:
              generationRef.current === generation
                ? await ops.record(state, report, elapsedMs)
                : state,
            value: null,
          }));

          // Уступить цикл событий: на web сборка идёт в главном потоке, и
          // между заметками ввод, перерисовка и «Отменить» обязаны успеть
          // обработаться (research.md §6).
          await new Promise((resolve) => setTimeout(resolve, 0));
        }
      })();
    },
    [client, running, mutate, remember],
  );

  const skip = useCallback(
    (notePath: string) => {
      const ops = client?.exportBatch;
      if (ops === undefined) return;
      void mutate(async (state) => ({ state: await ops.skip(state, notePath), value: null }));
    },
    [client, mutate],
  );

  const cancel = useCallback(() => {
    const ops = client?.exportBatch;
    if (ops === undefined) return;
    void mutate(async (state) => ({ state: await ops.cancel(state), value: null }));
  }, [client, mutate]);

  const clearBatch = useCallback(() => {
    if (running) return;
    stateRef.current = null;
    setBatch(null);
  }, [running]);

  // Смена vault (новый клиент) или закрытие рабочей области отменяет пакет
  // (FR-026). Текущая заметка дописывается атомарно или не пишется вовсе —
  // это гарантирует ядро, а не ожидание здесь.
  useEffect(() => {
    return () => {
      generationRef.current += 1;
      stateRef.current = null;
      setBatch(null);
      setLast(null);
      setRecent([]);
    };
  }, [client]);

  return {
    busy,
    last,
    dismissLast: useCallback(() => setLast(null), []),
    exportNote,
    recent,
    batch,
    batchRunning: running,
    startBatch,
    skip,
    cancel,
    clearBatch,
  };
}

/**
 * Итог для вызова, который не дошёл до ядра (оболочка ответила отказом:
 * vault закрыт, команда недоступна). Строка пакета становится ошибкой с этой
 * причиной, а не зависает «в работе».
 */
function thrown(notePath: string, cause: unknown): ExportReport {
  const message =
    typeof cause === "object" && cause !== null && "message" in cause
      ? String((cause as { message: unknown }).message)
      : String(cause);
  return {
    note_path: notePath,
    pdf_path: "",
    outcome: { kind: "CompileFailed" },
    diagnostics: [{ severity: "Error", message, location: undefined }],
  };
}
