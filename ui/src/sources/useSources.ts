"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { VaultClient } from "../vault-client.js";
import type {
  CitationScanView,
  CreateSourceFileError,
  LibrarySummary,
  VaultEvent,
} from "../wasm-core/typsidian_core.js";

// Ритм вызовов библиотеки источников (фича 017, contracts/frontend-sources.md
// § useSources). Правил здесь нет: что перечитать и что считается цитатой,
// решает ядро; хук только решает когда.

/** Заметок за шаг прохода индекса цитат (research.md §8). */
const SCAN_PORTION = 50;

export interface SourcesState {
  summary: LibrarySummary | null;
  scan: CitationScanView | null;
  /** Растёт при каждом изменении библиотеки или индекса — повод перечитать выдачу. */
  revision: number;
  refreshing: boolean;
  /** Первый показ панели: проход по заметкам порциями. */
  startScan(): void;
  cancelScan(): void;
  createFile(): Promise<CreateSourceFileError | null>;
  registerFile(path: string): Promise<boolean>;
  /** Событие vault — из общего потока рабочей области. */
  onEvent(event: VaultEvent): void;
}

function nextTask(): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, 0));
}

export function useSources(
  client: VaultClient | null,
  /** Пути YAML, указанные пользователем (`source_files` настроек vault). */
  registered: readonly string[],
  /** Записать `source_files`; `false` — ядро отклонило путь. */
  writeRegistered: (paths: string[]) => Promise<boolean>,
): SourcesState {
  const [summary, setSummary] = useState<LibrarySummary | null>(null);
  const [scan, setScan] = useState<CitationScanView | null>(null);
  const [revision, setRevision] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const latest = useRef({ client, registered });
  latest.current = { client, registered };
  const scanning = useRef(false);
  const scanStarted = useRef(false);
  const refreshAgain = useRef(false);
  const refreshRunning = useRef(false);

  const bump = useCallback(() => setRevision((value) => value + 1), []);

  // Обновление библиотеки: по файлу за вызов, уступая циклу событий между
  // файлами (plan.md → Complexity Tracking). Повторный запрос во время прохода
  // не теряется — проход повторится.
  const refresh = useCallback(async () => {
    if (refreshRunning.current) {
      refreshAgain.current = true;
      return;
    }
    refreshRunning.current = true;
    setRefreshing(true);
    try {
      do {
        refreshAgain.current = false;
        const current = latest.current.client;
        if (current === null) return;
        let changed = false;
        for (;;) {
          const step = await current.sources.refresh([...latest.current.registered]);
          changed ||= step.changed;
          if (step.pending === 0) break;
          await nextTask();
        }
        if (latest.current.client !== current) return;
        setSummary(await current.sources.summary());
        if (changed) bump();
      } while (refreshAgain.current);
    } catch {
      // Vault закрылся посреди обновления — следующий vault начнёт заново.
    } finally {
      refreshRunning.current = false;
      setRefreshing(false);
    }
  }, [bump]);

  const registeredKey = registered.join("\n");
  useEffect(() => {
    setSummary(null);
    setScan(null);
    scanStarted.current = false;
    if (client !== null) void refresh();
    return () => {
      // Смена vault: проход прежнего индекса не продолжается.
      if (scanning.current) void client?.sources.scanCancel();
      scanning.current = false;
    };
  }, [client, refresh]);
  useEffect(() => {
    if (client !== null) void refresh();
  }, [registeredKey, client, refresh]);

  const startScan = useCallback(() => {
    const current = latest.current.client;
    if (current === null || scanning.current) return;
    scanning.current = true;
    scanStarted.current = true;
    void (async () => {
      try {
        let view = await current.sources.scanStart();
        setScan(view);
        while (view.phase === "Running" && scanning.current && latest.current.client === current) {
          view = await current.sources.scanStep(SCAN_PORTION);
          setScan(view);
          await nextTask();
        }
        // Индекс знает пути `bibliography(…)` — YAML мог опознаться.
        await refresh();
        bump();
      } catch {
        // Vault закрылся во время прохода.
      } finally {
        scanning.current = false;
      }
    })();
  }, [bump, refresh]);

  const cancelScan = useCallback(() => {
    const current = latest.current.client;
    scanning.current = false;
    if (current !== null) void current.sources.scanCancel().then(setScan);
  }, []);

  const createFile = useCallback(async () => {
    const current = latest.current.client;
    if (current === null) return null;
    const result = await current.sources.createFile();
    // Событие vault придёт само, но список нужен сразу (T027).
    await refresh();
    return result.ok ? null : result.error;
  }, [refresh]);

  const registerFile = useCallback(
    async (path: string) => {
      const next = [...new Set([...latest.current.registered, path])];
      return writeRegistered(next);
    },
    [writeRegistered],
  );

  const onEvent = useCallback(
    (event: VaultEvent) => {
      const current = latest.current.client;
      if (current === null) return;
      switch (event.type) {
        case "SourceFilesChanged":
          void refresh();
          return;
        case "NoteCreated":
        case "NoteUpdated":
          if (scanStarted.current) void current.sources.noteChanged(event.path).then(bump);
          return;
        case "NoteDeleted":
          if (scanStarted.current) void current.sources.noteRemoved(event.path).then(bump);
          return;
        case "NoteRenamed":
          if (scanStarted.current) {
            void current.sources.noteRenamed(event.from_path, event.to_path).then(bump);
          }
          return;
        default:
          return;
      }
    },
    [bump, refresh],
  );

  return {
    summary,
    scan,
    revision,
    refreshing,
    startScan,
    cancelScan,
    createFile,
    registerFile,
    onEvent,
  };
}
