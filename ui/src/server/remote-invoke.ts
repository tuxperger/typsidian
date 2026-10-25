// Транспорт команд Tauri поверх HTTP к своему серверу (фича 037, research.md §5–§6).
//
// `invoke` — `POST /api/vaults/{vault}/invoke/{команда}` с аргументами JSON (или
// сырым телом для байтов вложения); ответ и отказ — те же значения, что вернула бы
// команда Tauri. `listen` — события vault из SSE `GET /api/vaults/{vault}/events`.
//
// Связь пропала — вызов ждёт и повторяется (`connection.ts`); после
// переподключения дерево сверяется с последним списком, и вкладка узнаёт об
// изменениях, пропущенных без связи.

import type { InvokeArgs } from "@tauri-apps/api/core";
import { withoutNullFields } from "../tauri-invoke.js";
import type { ShellTransport } from "../vault-client-tauri.js";
import type { Note, VaultEvent } from "../wasm-core/typsidian_core.js";
import type { ServerConnection } from "./connection.js";
import { DEMO_QUOTA_EVENT, unreachable, vaultPath } from "./server-api.js";

const VAULT_EVENTS = "vault://events";
const REINDEX_PROGRESS = "properties://reindex-progress";

type Handler = (event: { payload: unknown }) => void;

function randomId(): string {
  const bytes = new Uint8Array(12);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** Что изменилось между двумя списками заметок — события, как от внешней правки. */
export function listDifference(before: Note[], after: Note[]): VaultEvent[] {
  const old = new Map(before.map((note) => [note.path, JSON.stringify(note.stamp)]));
  const events: VaultEvent[] = [];
  for (const note of after) {
    const stamp = old.get(note.path);
    if (stamp === undefined) events.push({ type: "NoteCreated", path: note.path });
    else if (stamp !== JSON.stringify(note.stamp))
      events.push({ type: "NoteUpdated", path: note.path });
    old.delete(note.path);
  }
  for (const path of old.keys()) events.push({ type: "NoteDeleted", path });
  return events;
}

export function remoteTransport(vault: string, connection: ServerConnection): ShellTransport {
  // Id вкладки: свои правки сервер ей эхом не присылает.
  const client = randomId();
  const base = vaultPath(vault);
  const handlers = new Map<string, Set<Handler>>();
  let source: EventSource | null = null;
  let lastList: Note[] | null = null;
  let connectedOnce = false;
  let reconnectAttempt = 0;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  const emit = (event: string, payload: unknown): void => {
    for (const handler of handlers.get(event) ?? []) handler({ payload });
  };

  async function invoke<T = void>(command: string, args?: InvokeArgs): Promise<T> {
    const raw = args instanceof Uint8Array || args instanceof ArrayBuffer;
    const body = raw ? args : JSON.stringify(args ?? {});
    for (let attempt = 0; ; attempt++) {
      let response: Response;
      try {
        response = await fetch(`${base}/invoke/${encodeURIComponent(command)}`, {
          method: "POST",
          credentials: "same-origin",
          headers: {
            "Content-Type": raw ? "application/octet-stream" : "application/json",
            "X-Typsidian-CSRF": connection.session().csrf,
            "X-Typsidian-Client": client,
          },
          body: body as BodyInit,
        });
      } catch {
        await connection.waitOffline(attempt);
        continue;
      }
      if (unreachable(response)) {
        await connection.waitOffline(attempt);
        continue;
      }
      connection.online();
      if (response.status === 401) {
        // Сессия истекла или завершена: вызов дождётся нового входа (FR-010).
        await connection.waitSignedIn();
        attempt = -1;
        continue;
      }
      let value: unknown = null;
      try {
        value = await response.json();
      } catch {
        // Пустое тело — команда без ответа.
      }
      const result = withoutNullFields(value);
      if (!response.ok) {
        // Демо-объём исчерпан (фича 039): отказ видит не только вызвавший код.
        if ((result as { kind?: unknown } | null)?.kind === "DemoQuota") {
          window.dispatchEvent(new CustomEvent(DEMO_QUOTA_EVENT, { detail: result }));
        }
        throw result;
      }
      if (command === "list") lastList = result as Note[];
      return result as T;
    }
  }

  // ── События ──────────────────────────────────────────────────────────

  const resync = async (): Promise<void> => {
    const before = lastList;
    if (before === null) return;
    try {
      const after = await invoke<Note[]>("list");
      const events = listDifference(before, after);
      if (events.length > 0) emit(VAULT_EVENTS, events);
    } catch {
      // Vault пропал — следующий вызов скажет об этом сам.
    }
  };

  const scheduleReconnect = (): void => {
    if (reconnectTimer !== null || handlers.size === 0) return;
    const delay = Math.min(30_000, 1000 * 2 ** Math.min(reconnectAttempt, 5));
    reconnectAttempt += 1;
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      // Отказ подключения бывает и от истёкшей сессии: команда скажет точно.
      void invoke<string>("board_write_profile").catch(() => undefined);
      open();
    }, delay);
  };

  const open = (): void => {
    if (source !== null) return;
    const es = new EventSource(`${base}/events?client=${client}`, { withCredentials: true });
    source = es;
    es.addEventListener("ready", () => {
      reconnectAttempt = 0;
      connection.online();
      if (connectedOnce) void resync();
      connectedOnce = true;
    });
    const resyncAll = (): void => void resync();
    es.addEventListener("lagged", resyncAll);
    es.addEventListener("vault", (message) => {
      try {
        emit(VAULT_EVENTS, JSON.parse((message as MessageEvent<string>).data));
      } catch {
        // Не JSON — пропустить.
      }
    });
    es.addEventListener("progress", (message) => {
      try {
        emit(REINDEX_PROGRESS, JSON.parse((message as MessageEvent<string>).data));
      } catch {
        // Не JSON — пропустить.
      }
    });
    es.onerror = () => {
      // Браузер переподключается сам, пока поток не закрыт окончательно
      // (ответ не 200 — например, вход потерян).
      if (es.readyState === EventSource.CLOSED) {
        es.close();
        if (source === es) source = null;
        scheduleReconnect();
      }
    };
  };

  const close = (): void => {
    source?.close();
    source = null;
    if (reconnectTimer !== null) clearTimeout(reconnectTimer);
    reconnectTimer = null;
  };

  return {
    invoke,
    listen: <T>(event: string, handler: (event: { payload: T }) => void) => {
      const set = handlers.get(event) ?? new Set<Handler>();
      set.add(handler as Handler);
      handlers.set(event, set);
      open();
      return Promise.resolve(() => {
        set.delete(handler as Handler);
        if (set.size === 0) handlers.delete(event);
        if (handlers.size === 0) close();
      });
    },
  };
}
