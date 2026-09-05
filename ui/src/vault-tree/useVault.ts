import { useCallback, useMemo, useRef, useState } from "react";
import type {
  AccessState,
  Note,
  OpenError,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { bulkChangeReducer } from "./BulkChangeIndicator.js";
import { useVaultEvents, type VaultEventHandlers } from "./useVaultEvents.js";

// Принцип I конституции: этот хук не переизобретает vault-логику — он только
// вызывает ядро через `VaultClient` (contracts/vault-backend-trait.md) и
// реагирует на VaultEvent (contracts/frontend-events.md). Любая бизнес-логика
// живёт в ядре.
//
// Как именно клиент добирается до ядра — WASM в том же процессе или команды
// Tauri поверх `NativeBackend` — этот хук не знает и знать не должен (T113,
// T114): иначе в общем коде появилось бы ветвление по таргету.

/**
 * Причина, по которой не удалось открыть vault (FR-008). Варианты
 * соответствуют `OpenError` ядра там, где это возможно; на web папку выбирает
 * File System Access API, и он отвечает не `OpenError`, а `DOMException`,
 * поэтому классификация делается здесь — см. `classifyOpenFailure`.
 *
 * `PickerCancelled` ошибкой не считается (Assumptions): пользователь передумал.
 */
export type OpenVaultError =
  | { kind: "PickerCancelled" }
  | { kind: "PermissionDenied" }
  | { kind: "NotSupported" }
  /** FR-038: в папке больше заметок, чем поддерживается. */
  | { kind: "TooManyNotes" }
  | { kind: "OpenFailed"; message: string };

/**
 * Переводит отказ `showDirectoryPicker`/`fromHandle` в конкретную причину.
 * Общий текст «что-то пошло не так» запрещён (FR-008), поэтому неизвестный
 * отказ сохраняет исходное сообщение, а не прячет его.
 */
export function classifyOpenFailure(cause: unknown): OpenVaultError {
  if (cause instanceof DOMException) {
    if (cause.name === "AbortError") return { kind: "PickerCancelled" };
    // Пользователь отказал в доступе к папке либо доступ отозван политикой
    // браузера — для FR-008 это разные слова, чем «не удалось открыть».
    if (cause.name === "NotAllowedError" || cause.name === "SecurityError") {
      return { kind: "PermissionDenied" };
    }
  }
  // `fromHandle` отвергает типизированным `OpenError` из ядра, а не
  // `DOMException`: решение «эта папка не открывается и вот почему» принимает
  // ядро (принцип I), UI только подбирает формулировку.
  const openError = asOpenError(cause);
  if (openError) {
    switch (openError.kind) {
      case "TooManyNotes":
        return { kind: "TooManyNotes" };
      case "AccessDenied":
        return { kind: "PermissionDenied" };
      case "NotFound":
      case "NotADirectory":
        return { kind: "OpenFailed", message: openError.kind };
      default: {
        const exhaustive: never = openError;
        return exhaustive;
      }
    }
  }
  return { kind: "OpenFailed", message: String(cause) };
}

/** Сужение `unknown` до `OpenError` без `any` (принцип IV). */
function asOpenError(cause: unknown): OpenError | null {
  if (typeof cause !== "object" || cause === null || !("kind" in cause)) return null;
  const { kind } = cause as { kind: unknown };
  return kind === "NotFound" ||
    kind === "AccessDenied" ||
    kind === "NotADirectory" ||
    kind === "TooManyNotes"
    ? ({ kind } as OpenError)
    : null;
}

/** Чем закончилась попытка открыть vault. */
export type VaultOpenResult =
  | { ok: true; client: VaultClient }
  | { ok: false; error: OpenVaultError };

/**
 * Открытие vault — жест пользователя, и он платформенный: в браузере это
 * `showDirectoryPicker`, на рабочем столе — нативный диалог ОС (T115). Поэтому
 * жест передаётся оболочкой снаружи, а классификация отказов
 * ([`OpenVaultError`]) остаётся общей: пользователь обязан видеть одну и ту же
 * причину независимо от клиента.
 */
export type VaultOpener = () => Promise<VaultOpenResult>;

export interface VaultState {
  client: VaultClient | null;
  notes: Note[];
  accessState: AccessState | null;
  isLoading: boolean;
  error: OpenVaultError | null;
  /** Глубина вложенности пачек массовых изменений (US3 AS5). */
  bulkChangeDepth: number;
  /**
   * Растёт на каждое `LinksInvalidated` — сигнал пересчитать битые ссылки
   * (`useBrokenLinks`), не опрашивая ядро на каждом тике (FR-030).
   */
  linksRevision: number;
}

/**
 * FR-001, FR-007: открывает выбранную пользователем папку как vault.
 *
 * Сам жест выбора папки приходит извне ([`VaultOpener`]): в браузере это
 * `showDirectoryPicker`, на рабочем столе — нативный диалог ОС. Здесь — только
 * то, что одинаково для обоих: загрузка дерева, состояние доступа и реакция на
 * события.
 */
export function useVault(
  openClient: VaultOpener,
  handlers: VaultEventHandlers = {},
): {
  state: VaultState;
  openVault: () => Promise<void>;
  refresh: () => Promise<void>;
} {
  const [state, setState] = useState<VaultState>({
    client: null,
    notes: [],
    accessState: null,
    isLoading: false,
    error: null,
    bulkChangeDepth: 0,
    linksRevision: 0,
  });
  const clientRef = useRef<VaultClient | null>(null);

  const refresh = useCallback(async () => {
    const client = clientRef.current;
    if (!client) return;
    const notes = await client.list();
    const accessState = await client.accessState();
    setState((prev) => ({ ...prev, notes, accessState }));
  }, []);

  const openVault = useCallback(async () => {
    setState((prev) => ({ ...prev, isLoading: true, error: null }));
    const opened = await openClient();
    if (!opened.ok) {
      setState((prev) => ({ ...prev, isLoading: false, error: opened.error }));
      return;
    }
    const { client } = opened;
    clientRef.current = client;
    const notes = await client.list();
    const accessState = await client.accessState();
    setState({
      client,
      notes,
      accessState,
      isLoading: false,
      error: null,
      bulkChangeDepth: 0,
      linksRevision: 0,
    });
  }, [openClient]);

  // FR-016: подхватывать внешние изменения без перезапуска. Разбор событий и
  // сам цикл сверки живут в `useVaultEvents` — здесь только реакция на них:
  // обновление списка, состояния доступа, индикатора массовых изменений и
  // отметки о том, что битые ссылки пора пересчитать.
  //
  // Обработчики вызывающей стороны (конфликты, удаление с правками) не
  // перехватываются и не подменяются: `useVault` дополняет их своими, а не
  // решает за UI, что делать с конфликтом (принцип I).
  const eventHandlers = useMemo<VaultEventHandlers>(
    () => ({
      ...handlers,
      onNoteCreated: (path) => {
        handlers.onNoteCreated?.(path);
        void refresh();
      },
      onNoteUpdated: (path) => {
        handlers.onNoteUpdated?.(path);
        void refresh();
      },
      onNoteRenamed: (from, to) => {
        handlers.onNoteRenamed?.(from, to);
        void refresh();
      },
      onNoteDeleted: (path) => {
        handlers.onNoteDeleted?.(path);
        void refresh();
      },
      onNoteDeletedWithUnsavedEdits: (notePath) => {
        handlers.onNoteDeletedWithUnsavedEdits?.(notePath);
        void refresh();
      },
      onBulkChangeStarted: () => {
        handlers.onBulkChangeStarted?.();
        setState((prev) => ({
          ...prev,
          bulkChangeDepth: bulkChangeReducer(prev.bulkChangeDepth, "started"),
        }));
      },
      onBulkChangeSettled: () => {
        handlers.onBulkChangeSettled?.();
        setState((prev) => ({
          ...prev,
          bulkChangeDepth: bulkChangeReducer(prev.bulkChangeDepth, "settled"),
        }));
      },
      onAccessStateChanged: (accessState) => {
        handlers.onAccessStateChanged?.(accessState);
        setState((prev) => ({ ...prev, accessState }));
      },
      onLinksInvalidated: (affectedPaths) => {
        handlers.onLinksInvalidated?.(affectedPaths);
        setState((prev) => ({ ...prev, linksRevision: prev.linksRevision + 1 }));
      },
    }),
    [handlers, refresh],
  );

  useVaultEvents(state.client, eventHandlers);

  return { state, openVault, refresh };
}
