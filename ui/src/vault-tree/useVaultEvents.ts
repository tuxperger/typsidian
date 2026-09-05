import { useEffect, useRef } from "react";
import type {
  AccessState,
  FileStamp,
  VaultEvent,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";

// Принцип I конституции: этот файл ничего не решает про vault — он только
// разбирает уже готовые `VaultEvent` из ядра (contracts/frontend-events.md) и
// зовёт переданные обработчики. Никакой сверки с диском, никакого повторного
// определения «изменилось ли» здесь нет.

export interface VaultEventHandlers {
  onNoteCreated?: (path: string) => void;
  onNoteUpdated?: (path: string) => void;
  onNoteRenamed?: (fromPath: string, toPath: string) => void;
  onNoteDeleted?: (path: string) => void;
  /** FR-021: показать выбор из трёх вариантов, ничего не применяя. */
  onConflictDetected?: (notePath: string, diskSnapshot: FileStamp) => void;
  /** FR-025: предупредить и дать пересохранить. */
  onNoteDeletedWithUnsavedEdits?: (notePath: string) => void;
  /** US3 AS5: единый индикатор вместо шторма отдельных уведомлений. */
  onBulkChangeStarted?: () => void;
  onBulkChangeSettled?: () => void;
  /** FR-020, FR-036: не трактовать недоступность как удаление всех заметок. */
  onAccessStateChanged?: (accessState: AccessState) => void;
  /** FR-030: обновить индикацию битых ссылок для затронутых заметок. */
  onLinksInvalidated?: (affectedPaths: string[]) => void;
}

/**
 * Исчерпывающий разбор одного `VaultEvent` (contracts/frontend-events.md).
 * `never` в `default` — гейт принципа IV: если ядро добавит вариант события,
 * TS strict не даст собрать UI, пока его здесь не обработают.
 */
export function applyVaultEvent(
  event: VaultEvent,
  handlers: VaultEventHandlers,
): void {
  switch (event.type) {
    case "NoteCreated":
      handlers.onNoteCreated?.(event.path);
      return;
    case "NoteUpdated":
      handlers.onNoteUpdated?.(event.path);
      return;
    case "NoteRenamed":
      handlers.onNoteRenamed?.(event.from_path, event.to_path);
      return;
    case "NoteDeleted":
      handlers.onNoteDeleted?.(event.path);
      return;
    case "ConflictDetected":
      handlers.onConflictDetected?.(event.note_path, event.disk_snapshot);
      return;
    case "NoteDeletedWithUnsavedEdits":
      handlers.onNoteDeletedWithUnsavedEdits?.(event.note_path);
      return;
    case "BulkChangeStarted":
      handlers.onBulkChangeStarted?.();
      return;
    case "BulkChangeSettled":
      handlers.onBulkChangeSettled?.();
      return;
    case "VaultAccessChanged":
      handlers.onAccessStateChanged?.(event.access_state);
      return;
    case "LinksInvalidated":
      handlers.onLinksInvalidated?.(event.affected_paths);
      return;
    default: {
      const exhaustive: never = event;
      return exhaustive;
    }
  }
}

/**
 * Живое обновление дерева и редактора по внешним изменениям (T060, T114,
 * FR-016, FR-017): подписывается на поток `VaultEvent` и раздаёт события
 * обработчикам.
 *
 * Откуда берётся поток, здесь неизвестно, и это главное свойство этого хука:
 * web сверяется с диском по таймеру (у File System Access API нет подписки,
 * research.md §3), native получает push от `notify` через канал событий Tauri.
 * Разница живёт в реализации `VaultClient.subscribe`, а не в этой бизнес-логике
 * (принцип II).
 */
export function useVaultEvents(
  client: VaultClient | null,
  handlers: VaultEventHandlers,
): void {
  // Обработчики пересоздаются на каждый рендер; держим их в ref, чтобы не
  // пересоздавать подписку и не терять накопленные события на каждом рендере.
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  useEffect(() => {
    if (!client) return;
    return client.subscribe((events) => {
      for (const event of events) {
        applyVaultEvent(event, handlersRef.current);
      }
    });
  }, [client]);
}
