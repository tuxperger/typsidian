import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import type { VaultClient } from "@typsidian/ui/vault-client.js";
import type {
  VaultOpenResult,
  OpenVaultError,
} from "@typsidian/ui/vault-tree/useVault.js";
import type {
  AccessState,
  Board,
  BoardEvent,
  DiffLine,
  EditConflict,
  Embed,
  EmbedPlan,
  EmbedRef,
  FileKind,
  FileStamp,
  Note,
  VaultEvent,
} from "@typsidian/ui/wasm-core/typsidian_core.js";

// Реализация `VaultClient` для рабочего стола (001 T113, T114, T115).
//
// Каждый метод — один `invoke` соответствующей команды Tauri, за которой стоит
// `NativeBackend` (`apps/desktop/src-tauri/src/lib.rs`). Правил здесь нет: их
// нельзя завести, не продублировав ядро (принцип I).
//
// Типы взяты из тех же сгенерированных из Rust определений, что и на web
// (принцип IV): импорт типовой, WASM в сборку оболочки не попадает — ядро здесь
// нативное.

/** Имя события, которым оболочка отдаёт окну пачку `VaultEvent`. */
const VAULT_EVENTS = "vault://events";

/**
 * Открыть vault нативным диалогом ОС (T115).
 *
 * Классификация отказов та же, что у web (`OpenVaultError`): отмена диалога
 * ошибкой не считается — пользователь передумал (Assumptions). Причины, которых
 * на рабочем столе не бывает (`NotSupported` — нет File System Access API),
 * здесь просто не возникают, а `TooManyNotes` приходит из того же ядра.
 */
export async function openTauriVault(): Promise<VaultOpenResult> {
  let chosen: string | null;
  try {
    const selection = await openFolderDialog({
      directory: true,
      multiple: false,
      title: "Выберите папку с заметками",
    });
    chosen = typeof selection === "string" ? selection : null;
  } catch (cause) {
    return { ok: false, error: { kind: "OpenFailed", message: String(cause) } };
  }
  // `null` — диалог закрыт без выбора.
  if (chosen === null) return { ok: false, error: { kind: "PickerCancelled" } };

  try {
    await invoke<void>("open_vault", { path: chosen });
  } catch (cause) {
    return { ok: false, error: classifyNativeOpenFailure(cause) };
  }
  return { ok: true, client: tauriVaultClient() };
}

/**
 * Отказ открытия от ядра — теми же вариантами, что на web.
 *
 * `OpenError` пересекает границу как размеченное объединение, поэтому здесь
 * сужение по `kind`, а не разбор строки.
 */
function classifyNativeOpenFailure(cause: unknown): OpenVaultError {
  switch (kindOf(cause)) {
    case "TooManyNotes":
      return { kind: "TooManyNotes" };
    case "AccessDenied":
      return { kind: "PermissionDenied" };
    case "NotFound":
      return { kind: "OpenFailed", message: "NotFound" };
    case "NotADirectory":
      return { kind: "OpenFailed", message: "NotADirectory" };
    default:
      return { kind: "OpenFailed", message: String(cause) };
  }
}

function kindOf(cause: unknown): string | null {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const { kind } = cause as { kind: unknown };
    return typeof kind === "string" ? kind : null;
  }
  return null;
}

export function tauriVaultClient(): VaultClient {
  // Снимок «есть ли что терять» для `beforeunload`, где ждать промис нельзя.
  //
  // Обновляется двумя путями: пессимистично — сразу при объявлении правок, и
  // точно — после каждой операции, которая могла их снять. Ошибаться этому
  // снимку позволено только в сторону «есть несохранённое» (см. `VaultClient`),
  // поэтому `beginEdit` ставит флаг синхронно, не дожидаясь ответа ядра.
  let unresolved = false;

  const refreshUnresolved = async (): Promise<string[]> => {
    const paths = await invoke<string[]>("unresolved_edit_paths");
    unresolved = paths.length > 0;
    return paths;
  };

  return {
    list: () => invoke<Note[]>("list"),
    read: (path) => invoke<string>("read", { path }),
    write: async (path, content, expected) => {
      await invoke<void>("write", { path, content, expected: expected ?? null });
      await refreshUnresolved();
    },
    create: (path) => invoke<void>("create", { path }),
    rename: (from, to) => invoke<void>("rename", { from, to }),
    delete: (path) => invoke<void>("delete", { path }),
    backlinks: (path) => invoke<Note[]>("backlinks", { path }),
    brokenLinks: (path) => invoke<string[]>("broken_links", { path }),
    accessState: () => invoke<AccessState>("access_state"),
    fileKind: (path) => invoke<FileKind>("file_kind", { path }),

    beginEdit: async (path, buffer) => {
      unresolved = true;
      await invoke<void>("begin_edit", { path, buffer });
    },
    pendingConflict: async (path) => {
      const conflict = await invoke<EditConflict | null>("pending_conflict", { path });
      return conflict ?? undefined;
    },
    conflictDiff: (path) => invoke<DiffLine[]>("conflict_diff", { path }),
    resolveConflictKeepLocal: async (path) => {
      await invoke<void>("resolve_conflict_keep_local", { path });
      await refreshUnresolved();
    },
    resolveConflictAcceptExternal: async (path) => {
      const external = await invoke<string | null>("resolve_conflict_accept_external", {
        path,
      });
      await refreshUnresolved();
      return external ?? undefined;
    },
    unresolvedEditPaths: refreshUnresolved,
    hasUnresolvedEdits: () => unresolved,

    subscribe: (onEvents) => {
      // Push, а не опрос: ядро сигналит появление событий, оболочка пересылает
      // их в окно одним `emit` (T114). Подписка асинхронна, поэтому отписка
      // дожидается её же промиса — иначе гонка при быстром размонтировании
      // оставила бы слушателя навсегда.
      let stop: (() => void) | null = null;
      let cancelled = false;
      void listen<VaultEvent[]>(VAULT_EVENTS, (event) => {
        onEvents(event.payload);
        // Правки могли быть сняты извне (файл удалён, конфликт разрешён в
        // другом окне) — снимок обязан это догнать.
        void refreshUnresolved();
      }).then((unlisten) => {
        if (cancelled) unlisten();
        else stop = unlisten;
      });
      return () => {
        cancelled = true;
        stop?.();
      };
    },

    createBoard: (path) => invoke<Board>("create_board", { path }),
    readBoard: (path) => invoke<string>("read_board", { path }),
    writeBoard: (path, content, expected) =>
      invoke<void>("write_board", { path, content, expected: expected ?? null }),
    writeSnapshot: (snapshotPath, svg, boardPath, boardContent) =>
      invoke<void>("write_snapshot", {
        snapshotPath,
        svg,
        boardPath,
        boardContent,
      }),
    embedsOf: (notePath, noteText) =>
      invoke<Embed[]>("embeds_of", { notePath, noteText }),
    embedsInText: (notePath, noteText) =>
      invoke<EmbedRef[]>("embeds_in_text", { notePath, noteText }),
    embedsChanged: async (notePath, noteText, changed) => {
      const event = await invoke<BoardEvent | null>("embeds_changed", {
        notePath,
        noteText,
        changed,
      });
      return event ?? undefined;
    },
    planEmbed: (notePath, noteText, boardPath) =>
      invoke<EmbedPlan>("plan_embed", { notePath, noteText, boardPath }),
    embedMarkup: (plan) => invoke<string>("embed_markup", { plan }),
  };
}

/**
 * `FileStamp` в сигнатурах выше приходит из тех же типов ядра; отдельного
 * объявления здесь нет намеренно — тип используется как есть.
 */
export type { FileStamp };
