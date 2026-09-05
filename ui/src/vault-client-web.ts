import init, { WebBackend } from "./wasm-core/typsidian_core.js";
import type { VaultClient } from "./vault-client.js";
import { classifyOpenFailure, type VaultOpenResult } from "./vault-tree/useVault.js";

// Реализация `VaultClient` для web: ядро в том же процессе, WASM
// (`packages/core` → `wasm-pack --target web`).
//
// Единственное место в проекте, где `WebBackend` импортируется **значением**, а
// не типом. Остальной общий UI работает через `VaultClient`, поэтому оболочка
// рабочего стола не втягивает WASM в свою сборку: она этот модуль не импортирует
// вовсе, а `import type` из `wasm-core` стирается компилятором.

/**
 * Интервал сверки с диском. У File System Access API нет подписки на изменения
 * (research.md §3), поэтому web опрашивает ядро периодически; значение выбрано
 * под бюджет SC-004 «внешнее изменение видно ≤2 с»: один тик на обнаружение
 * плюс запас на сам обход.
 */
export const VAULT_POLL_INTERVAL_MS = 1000;

let wasmInitPromise: Promise<unknown> | null = null;

function ensureWasmInitialized(): Promise<unknown> {
  if (!wasmInitPromise) wasmInitPromise = init();
  return wasmInitPromise;
}

/** Оборачивает готовый `WebBackend` в общий интерфейс. */
export function webVaultClient(backend: WebBackend): VaultClient {
  return {
    list: () => backend.list(),
    read: (path) => backend.read(path),
    write: (path, content, expected) => backend.write(path, content, expected),
    create: (path) => backend.create(path),
    rename: (from, to) => backend.rename(from, to),
    delete: (path) => backend.delete(path),
    backlinks: (path) => backend.backlinks(path),
    brokenLinks: (path) => backend.brokenLinks(path),
    accessState: async () => backend.accessState(),
    fileKind: async (path) => WebBackend.fileKind(path),

    beginEdit: (path, buffer) => backend.beginEdit(path, buffer),
    pendingConflict: async (path) => backend.pendingConflict(path),
    conflictDiff: (path) => backend.conflictDiff(path),
    resolveConflictKeepLocal: (path) => backend.resolveConflictKeepLocal(path),
    resolveConflictAcceptExternal: (path) =>
      backend.resolveConflictAcceptExternal(path),
    unresolvedEditPaths: async () => backend.unresolvedEditPaths(),
    // Ядро в том же процессе, поэтому снимок не нужен: ответ всегда точен.
    hasUnresolvedEdits: () => backend.unresolvedEditPaths().length > 0,

    subscribe: (onEvents) => {
      let cancelled = false;
      // Один цикл сверки может не успеть за интервал на большом vault — без
      // этого флага вызовы наложились бы друг на друга.
      let inFlight = false;

      const tick = async (): Promise<void> => {
        if (inFlight || cancelled) return;
        inFlight = true;
        try {
          // Порядок из двух шагов обязателен и продиктован ядром: `pollEvents`
          // сам ничего не обнаруживает — очередь наполняет `pollForChanges`
          // (doc-комментарий `WebBackend::poll_for_changes`). Без первого шага
          // ни одно внешнее изменение не было бы замечено.
          await backend.pollForChanges();
          if (cancelled) return;
          const events = backend.pollEvents();
          if (events.length > 0) onEvents(events);
        } finally {
          inFlight = false;
        }
      };

      const timer = window.setInterval(() => void tick(), VAULT_POLL_INTERVAL_MS);
      return () => {
        cancelled = true;
        window.clearInterval(timer);
      };
    },

    createBoard: (path) => backend.createBoard(path),
    readBoard: (path) => backend.readBoard(path),
    writeBoard: (path, content, expected) =>
      backend.writeBoard(path, content, expected),
    writeSnapshot: (snapshotPath, svg, boardPath, boardContent) =>
      backend.writeSnapshot(snapshotPath, svg, boardPath, boardContent),
    embedsOf: (notePath, noteText) => backend.embedsOf(notePath, noteText),
    embedsInText: async (notePath, noteText) =>
      WebBackend.embedsInText(notePath, noteText),
    embedsChanged: async (notePath, noteText, changed) =>
      WebBackend.embedsChanged(notePath, noteText, changed),
    planEmbed: async (notePath, noteText, boardPath) =>
      WebBackend.planEmbed(notePath, noteText, boardPath),
    embedMarkup: async (plan) => WebBackend.embedMarkup(plan),
  };
}

/**
 * Открыть vault в браузере (FR-001, FR-008).
 *
 * `showDirectoryPicker` — жест пользователя, поэтому вызывается здесь, в UI, а
 * не в ядре: ядро получает уже готовый хендл.
 */
export async function openWebVault(): Promise<VaultOpenResult> {
  try {
    await ensureWasmInitialized();
    const picker = (
      window as unknown as {
        showDirectoryPicker?: () => Promise<FileSystemDirectoryHandle>;
      }
    ).showDirectoryPicker;
    if (!picker) return { ok: false, error: { kind: "NotSupported" } };

    const handle = await picker();
    const backend = await WebBackend.fromHandle(handle.name, handle);
    return { ok: true, client: webVaultClient(backend) };
  } catch (cause) {
    return { ok: false, error: classifyOpenFailure(cause) };
  }
}
