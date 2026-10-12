import { useCallback, useRef, useState } from "react";
import type { Translate } from "../i18n/index.js";
import type { VaultClient } from "../vault-client.js";
import type {
  MoveOutcome,
  MoveRequest,
  Note,
  PathChange,
} from "../wasm-core/typsidian_core.js";

// Единственный путь переноса в интерфейсе (фича 023,
// contracts/frontend-tree-move.md): им пользуются жест, «Переместить в…» и
// отмена. Что допустимо и что сломается, решает ядро — здесь только порядок
// шагов: влияние → подтверждение → перенос → итог.

/** Что переносится: путь и имя для сообщений. */
export interface MoveEntryRef {
  path: string;
  name: string;
}

export type MoveNotice =
  | { kind: "done"; entry: MoveEntryRef; folder: string; undo: MoveRequest }
  | { kind: "undone" }
  | { kind: "failed"; message: (t: Translate) => string };

/** Запрос подтверждения переноса, разрывающего ссылки (FR-016, FR-017). */
export interface MoveConfirmRequest {
  entry: MoveEntryRef;
  folder: string;
  affected: Note[];
}

export interface MoveEntryOptions {
  client: VaultClient;
  /** Перечитать дерево — обычно `useVault().refresh`. */
  onNotesChanged: () => void | Promise<void>;
  /** Открытый файл и конфликты следуют за путём (FR-020). */
  onPathsMoved?: ((moved: PathChange[]) => void) | undefined;
  /** Свёрнутость папок следует за путём (FR-020). */
  onFoldersMoved: (folders: PathChange[]) => void;
}

function folderOf(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash === -1 ? "" : path.slice(0, slash);
}

/**
 * Причина отказа из ядра. Ошибка переноса приходит либо отказом
 * (`MoveRefusal`), либо сбоем записи; форма различается на границе wasm и
 * Tauri, поэтому разбирается по признаку `kind`, а не по типу.
 */
function failureOf(cause: unknown): { kind: string; name?: string } {
  let value: unknown = cause;
  for (let depth = 0; depth < 3; depth += 1) {
    if (typeof value !== "object" || value === null) break;
    const record = value as Record<string, unknown>;
    const kind = record["kind"];
    if (kind === "Refused") {
      value = record["refusal"] ?? record["reason"] ?? record["value"];
      continue;
    }
    if (typeof kind === "string") {
      const name = record["name"];
      return typeof name === "string" ? { kind, name } : { kind };
    }
    break;
  }
  return { kind: "Io" };
}

export function useMoveEntry({
  client,
  onNotesChanged,
  onPathsMoved,
  onFoldersMoved,
}: MoveEntryOptions) {
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [notice, setNotice] = useState<MoveNotice | null>(null);
  const [confirm, setConfirm] = useState<MoveConfirmRequest | null>(null);
  const confirmResolver = useRef<((allowed: boolean) => void) | null>(null);
  // Занятость в ref, а не только в state: два броска подряд успевают до
  // перерисовки, и второй обязан увидеть первый.
  const busyRef = useRef(false);
  // Флаг отмены держит интерфейс: вызов ядра занят до конца переноса, и
  // отдельным вызовом его не прервать (research.md §6, принцип X).
  const abortRef = useRef<AbortController | null>(null);

  const folderLabel = useCallback(
    (folder: string) => (folder === "" ? client.rootName : folder),
    [client],
  );

  const answerConfirm = useCallback((allowed: boolean) => {
    confirmResolver.current?.(allowed);
    confirmResolver.current = null;
    setConfirm(null);
  }, []);

  const describeFailure = useCallback(
    (cause: unknown, entry: MoveEntryRef, toFolder: string, undo: boolean) => {
      const failure = failureOf(cause);
      const folder = folderLabel(toFolder);
      const name = failure.name ?? entry.name;
      return (t: Translate): string => {
        switch (failure.kind) {
          case "NameConflict":
            return undo
              ? t("tree.move.undoRefused", { folder, name })
              : t("tree.move.refused.nameConflict", { folder, name });
          case "AccessDenied":
            return t("tree.move.refused.accessDenied");
          case "NotFound":
            return t("tree.move.refused.notFound", { name: entry.name });
          case "TargetNotFound":
            return t("tree.move.refused.targetNotFound", { folder });
          default:
            return t("tree.move.refused.io", { name: entry.name });
        }
      };
    },
    [folderLabel],
  );

  const run = useCallback(
    async (entry: MoveEntryRef, toFolder: string, undo: boolean): Promise<void> => {
      if (busyRef.current) return;
      busyRef.current = true;
      setBusy(true);
      setNotice(null);
      const request: MoveRequest = { from: entry.path, to_folder: toFolder };
      try {
        if (!undo) {
          // Отмена восстанавливает ссылки, поэтому о них не спрашивает
          // (research.md §7).
          const impact = await client.moveImpact(request);
          if (impact.broken.length > 0) {
            const allowed = await new Promise<boolean>((resolve) => {
              confirmResolver.current = resolve;
              setConfirm({ entry, folder: folderLabel(toFolder), affected: impact.broken });
            });
            if (!allowed) return;
          }
        }
        // Набранное дописывается до переноса: файл переезжает уже с ним, и
        // запись, запланированная на старый путь, не выстрелит после.
        await client.flushAll();
        const abort = new AbortController();
        abortRef.current = abort;
        const outcome: MoveOutcome = await client.moveEntry(request, {
          onProgress: (done, total) => setProgress({ done, total }),
          signal: abort.signal,
        });
        if (outcome.kind === "Cancelled") return;
        onPathsMoved?.(outcome.moved);
        onFoldersMoved(outcome.folders);
        await onNotesChanged();
        if (outcome.kind === "Partial") {
          const count = outcome.left.length;
          const folder = folderLabel(folderOf(entry.path));
          setNotice({
            kind: "failed",
            message: (t) => t("tree.move.partial", { count, folder }),
          });
        } else if (undo) {
          setNotice({ kind: "undone" });
        } else {
          setNotice({ kind: "done", entry, folder: folderLabel(toFolder), undo: outcome.undo });
        }
      } catch (cause) {
        // Элемент уже лежит в цели — пустое действие без сообщений (FR-005).
        if (failureOf(cause).kind === "SamePlace") return;
        setNotice({ kind: "failed", message: describeFailure(cause, entry, toFolder, undo) });
        // Отказ мог означать, что дерево устарело (элемента или цели уже нет).
        await onNotesChanged();
      } finally {
        abortRef.current = null;
        busyRef.current = false;
        setBusy(false);
        setProgress(null);
      }
    },
    [client, describeFailure, folderLabel, onFoldersMoved, onNotesChanged, onPathsMoved],
  );

  const requestMove = useCallback(
    (entry: MoveEntryRef, toFolder: string) => run(entry, toFolder, false),
    [run],
  );

  const undo = useCallback(() => {
    if (notice?.kind !== "done") return Promise.resolve();
    const request = notice.undo;
    return run({ path: request.from, name: notice.entry.name }, request.to_folder, true);
  }, [notice, run]);

  /** Прервать идущий перенос: скопированное удаляется, vault остаётся прежним. */
  const cancel = useCallback(() => abortRef.current?.abort(), []);

  const dismissNotice = useCallback(() => setNotice(null), []);
  const showNotice = useCallback((next: MoveNotice) => setNotice(next), []);

  return {
    busy,
    progress,
    notice,
    confirm,
    answerConfirm,
    requestMove,
    undo,
    cancel,
    dismissNotice,
    showNotice,
  };
}

export type MoveEntryController = ReturnType<typeof useMoveEntry>;
