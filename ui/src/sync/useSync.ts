"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type {
  Keeper,
  SyncError,
  SyncEvent,
  SyncOverview,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { asSyncError, type SyncClient } from "./sync-client.js";

// Синхронизация со стороны интерфейса (фича 011, contracts/frontend-sync.md).
//
// Живёт в `Workspace`, а не в панели: индикатор виден и при закрытой панели,
// а автоматика (автоснимок, автообмен) обязана идти, пока vault открыт. Правил
// здесь нет (принцип I): хук держит сводку, крутит таймер такта и передаёт
// события — что и когда делать, решает `SyncSession` ядра.

/** Самый частый такт: чаще ядро и не просит, а папку обходить не бесплатно. */
const MIN_TICK_MS = 2_000;
/** Самый редкий такт — чтобы состояние Syncthing и конфликтные копии не устаревали. */
const MAX_TICK_MS = 60_000;
/** Пауза после события vault перед перечитыванием сводки. */
const REFRESH_DEBOUNCE_MS = 1_500;
/** Не чаще этого ядру сообщается об изменении файлов. */
const NOTE_CHANGE_THROTTLE_MS = 1_000;

/** Что сейчас выполняется по просьбе пользователя — для индикатора и блокировки кнопок. */
export type SyncActivity = "enable" | "snapshot" | "push" | "pull" | "connect" | "resolve";

/** Действие на не-хранителе ждёт подтверждения (FR-030). */
export interface KeeperConfirmation {
  /** `null` — хранитель оспаривается, назвать некого. */
  keeper: Keeper | null;
  confirm: () => void;
  cancel: () => void;
}

export interface SyncControl {
  client: SyncClient;
  /** `null` — сводка ещё не прочитана. */
  overview: SyncOverview | null;
  activity: SyncActivity | null;
  /** Последний отказ — фонового обмена или действия пользователя (FR-019). */
  error: SyncError | null;
  dismissError: () => void;
  /** Растёт с каждым изменением истории: списки перечитываются по нему. */
  revision: number;
  refresh: () => Promise<void>;
  /**
   * Выполняет действие: отмечает занятость, ловит отказ в `error`, затем
   * перечитывает сводку. Возвращает результат или `undefined` при отказе.
   */
  run: <T>(activity: SyncActivity, op: () => Promise<T>) => Promise<T | undefined>;
  /**
   * То же, но для действий, которые ядро не даёт делать не-хранителю без
   * подтверждения (снимок, отправка, получение): при `KeeperElsewhere` или
   * `KeeperDisputed` спрашивает пользователя и повторяет с подтверждением.
   */
  runGated: <T>(
    activity: SyncActivity,
    op: (confirmNotKeeper: boolean) => Promise<T>,
  ) => Promise<T | undefined>;
  keeperConfirmation: KeeperConfirmation | null;
  /** Снимок с сохранением открытых заметок перед ним (FR-006). */
  snapshot: (paths: string[] | null, message: string) => Promise<boolean>;
  push: () => Promise<void>;
  pull: () => Promise<void>;
  cancel: () => void;
  /** Событие vault: отсчёт паузы автоснимка и отложенное перечитывание сводки. */
  noticeVaultChange: () => void;
}

export interface SyncHandlers {
  /** Файлы переписаны получением или откатом — открытые обновляются (001). */
  onFilesChanged: (paths: string[]) => void;
}

export function useSync(
  client: SyncClient | null,
  vault: VaultClient | null,
  autoPrefix: string,
  handlers: SyncHandlers,
): SyncControl | null {
  const [overview, setOverview] = useState<SyncOverview | null>(null);
  const [activity, setActivity] = useState<SyncActivity | null>(null);
  const [error, setError] = useState<SyncError | null>(null);
  const [revision, setRevision] = useState(0);
  const [keeperConfirmation, setKeeperConfirmation] = useState<KeeperConfirmation | null>(
    null,
  );

  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;
  const autoPrefixRef = useRef(autoPrefix);
  autoPrefixRef.current = autoPrefix;
  // Поколение: смена vault останавливает цикл и ответы прежнего клиента.
  const generationRef = useRef(0);

  const drainEvents = useCallback(async (): Promise<boolean> => {
    if (client === null) return false;
    let events: SyncEvent[];
    try {
      events = await client.pollEvents();
    } catch {
      return false;
    }
    let changed = false;
    for (const event of events) {
      switch (event.kind) {
        case "OverviewChanged":
        case "AutoSnapshotDone":
          changed = true;
          break;
        case "ExchangeFailed":
          setError(event.error);
          changed = true;
          break;
        case "FilesChanged":
          handlersRef.current.onFilesChanged(event.paths);
          changed = true;
          break;
        case "Progress":
          break;
        default: {
          const exhaustive: never = event;
          return exhaustive;
        }
      }
    }
    return changed;
  }, [client]);

  const refresh = useCallback(async () => {
    if (client === null) return;
    const generation = generationRef.current;
    try {
      const next = await client.overview();
      if (generationRef.current !== generation) return;
      setOverview(next);
      setRevision((count) => count + 1);
    } catch (cause) {
      if (generationRef.current === generation) setError(asSyncError(cause));
    }
  }, [client]);

  const run = useCallback(
    async <T,>(kind: SyncActivity, op: () => Promise<T>): Promise<T | undefined> => {
      setActivity(kind);
      setError(null);
      try {
        return await op();
      } catch (cause) {
        const failure = asSyncError(cause);
        // Отмена — решение пользователя, а не отказ: сообщать о ней нечего.
        if (failure.kind !== "Cancelled") setError(failure);
        return undefined;
      } finally {
        setActivity(null);
        await drainEvents();
        await refresh();
      }
    },
    [drainEvents, refresh],
  );

  const runGated = useCallback(
    async <T,>(
      kind: SyncActivity,
      op: (confirmNotKeeper: boolean) => Promise<T>,
    ): Promise<T | undefined> => {
      setActivity(kind);
      setError(null);
      try {
        try {
          return await op(false);
        } catch (cause) {
          const failure = asSyncError(cause);
          if (failure.kind !== "KeeperElsewhere" && failure.kind !== "KeeperDisputed") {
            throw cause;
          }
          const confirmed = await new Promise<boolean>((resolve) => {
            setKeeperConfirmation({
              keeper: failure.kind === "KeeperElsewhere" ? failure.keeper : null,
              confirm: () => resolve(true),
              cancel: () => resolve(false),
            });
          });
          setKeeperConfirmation(null);
          if (!confirmed) return undefined;
          return await op(true);
        }
      } catch (cause) {
        const failure = asSyncError(cause);
        if (failure.kind !== "Cancelled") setError(failure);
        return undefined;
      } finally {
        setActivity(null);
        await drainEvents();
        await refresh();
      }
    },
    [drainEvents, refresh],
  );

  const snapshot = useCallback(
    async (paths: string[] | null, message: string): Promise<boolean> => {
      if (client === null) return false;
      const done = await runGated("snapshot", async (confirmNotKeeper) => {
        // Сохранение — здесь, а не в ядре: буферы держат редакторы. Что
        // значит «сохранено», при этом решает ядро (`flushAll` спрашивает его).
        const flushed = vault === null ? true : await vault.flushAll();
        return client.snapshot({
          paths,
          message,
          autoPrefix: autoPrefixRef.current,
          flushed,
          confirmNotKeeper,
        });
      });
      return done !== undefined;
    },
    [client, vault, runGated],
  );

  const push = useCallback(async () => {
    if (client === null) return;
    await runGated("push", (confirm) => client.push(confirm));
  }, [client, runGated]);

  const pull = useCallback(async () => {
    if (client === null) return;
    await runGated("pull", (confirm) => client.pull(confirm));
  }, [client, runGated]);

  // Таймер такта (T062). Только вызывает ядро: когда следующий такт, говорит
  // его ответ (`next_tick_at`), здесь он лишь зажимается в разумные границы.
  useEffect(() => {
    if (client === null) return;
    const generation = ++generationRef.current;
    let timer: number | undefined;
    let stopped = false;

    const loop = async (): Promise<void> => {
      if (stopped) return;
      let delay = MAX_TICK_MS;
      try {
        const outcome = await client.tick(autoPrefixRef.current);
        delay = outcome.next_tick_at - Date.now();
      } catch {
        // Такт не состоялся (vault закрывается, оболочка занята) — следующий
        // придёт по расписанию.
      }
      if (stopped || generationRef.current !== generation) return;
      await drainEvents();
      await refresh();
      if (stopped) return;
      timer = window.setTimeout(
        () => void loop(),
        Math.min(MAX_TICK_MS, Math.max(MIN_TICK_MS, delay)),
      );
    };

    setOverview(null);
    setError(null);
    setActivity(null);
    void loop();
    return () => {
      stopped = true;
      generationRef.current += 1;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [client, drainEvents, refresh]);

  const debounceRef = useRef<number | undefined>(undefined);
  const lastNoteChangeRef = useRef(0);
  const noticeVaultChange = useCallback(() => {
    if (client === null) return;
    // Набор текста сообщает об изменении на каждое нажатие; ядру достаточно
    // знать момент с точностью до секунды.
    const now = Date.now();
    if (now - lastNoteChangeRef.current >= NOTE_CHANGE_THROTTLE_MS) {
      lastNoteChangeRef.current = now;
      void client.noteChange().catch(() => undefined);
    }
    if (debounceRef.current !== undefined) window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(() => void refresh(), REFRESH_DEBOUNCE_MS);
  }, [client, refresh]);
  useEffect(
    () => () => {
      if (debounceRef.current !== undefined) window.clearTimeout(debounceRef.current);
    },
    [client],
  );

  const dismissError = useCallback(() => setError(null), []);
  const cancel = useCallback(() => client?.cancel(), [client]);

  if (client === null) return null;
  return {
    client,
    overview,
    activity,
    error,
    dismissError,
    revision,
    refresh,
    run,
    runGated,
    keeperConfirmation,
    snapshot,
    push,
    pull,
    cancel,
    noticeVaultChange,
  };
}
