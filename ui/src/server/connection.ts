// Связь со своим сервером (фича 037, FR-010, макет S5): есть, нет (повтор с
// нарастающей паузой) или вход потерян (сессия истекла или завершена).
//
// Вызов, который не дошёл до сервера, не падает: он ждёт здесь, пока связь
// вернётся или пользователь снова войдёт, и повторяется. Поэтому правка остаётся в
// редакторе и сохраняется после восстановления (SC-004), а рабочая область не
// размонтируется на время входа.

import type { ServerSession } from "./server-api.js";

export type ConnectionState =
  | { kind: "online" }
  /** `retryAt` — время следующей попытки, мс Unix. */
  | { kind: "offline"; retryAt: number }
  | { kind: "signedOut" };

/** Паузы между попытками: 1, 2, 4… с, не больше 30 с. */
export function retryDelayMs(attempt: number): number {
  return Math.min(30_000, 1000 * 2 ** Math.min(attempt, 5));
}

export interface ServerConnection {
  /** Сессия, от имени которой идут вызовы; меняется после повторного входа. */
  session(): ServerSession;
  state(): ConnectionState;
  subscribe(listener: (state: ConnectionState) => void): () => void;
  /** Вызов не дошёл: ждать паузу `attempt` (или «Повторить»). */
  waitOffline(attempt: number): Promise<void>;
  /** Сессии больше нет: ждать нового входа. */
  waitSignedIn(): Promise<void>;
  online(): void;
  /** Новый вход после истёкшей сессии — ждущие вызовы идут дальше. */
  resume(session: ServerSession): void;
  /** «Повторить» в баннере. */
  retryNow(): void;
}

export function createServerConnection(initial: ServerSession): ServerConnection {
  let session = initial;
  let state: ConnectionState = { kind: "online" };
  const listeners = new Set<(state: ConnectionState) => void>();
  let wakeOffline: (() => void)[] = [];
  let wakeSignedIn: (() => void)[] = [];

  const set = (next: ConnectionState): void => {
    if (next.kind === state.kind && next.kind !== "offline") return;
    state = next;
    for (const listener of listeners) listener(state);
  };

  const flush = (queue: (() => void)[]): void => {
    for (const wake of queue) wake();
  };

  return {
    session: () => session,
    state: () => state,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    waitOffline: (attempt) =>
      new Promise<void>((resolve) => {
        const delay = retryDelayMs(attempt);
        if (state.kind !== "signedOut") set({ kind: "offline", retryAt: Date.now() + delay });
        const timer = setTimeout(() => {
          wakeOffline = wakeOffline.filter((wake) => wake !== done);
          resolve();
        }, delay);
        const done = (): void => {
          clearTimeout(timer);
          resolve();
        };
        wakeOffline.push(done);
      }),
    waitSignedIn: () =>
      new Promise<void>((resolve) => {
        set({ kind: "signedOut" });
        wakeSignedIn.push(resolve);
      }),
    online: () => {
      if (state.kind === "offline") set({ kind: "online" });
    },
    resume: (next) => {
      session = next;
      set({ kind: "online" });
      const queue = wakeSignedIn;
      wakeSignedIn = [];
      flush(queue);
    },
    retryNow: () => {
      const queue = wakeOffline;
      wakeOffline = [];
      flush(queue);
    },
  };
}
