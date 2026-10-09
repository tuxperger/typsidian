// Владелец Web Worker сборки (005 FR-007, FR-026, T022, T023, T030).
//
// Ядро отдаёт сюда сообщение одного прохода компилятора и ждёт ответ
// (`PreviewEngine.setCompileWorker`). Здесь только то, что умеет один браузер:
// создать воркер, дождаться ответа и убить воркер, если сборка не уложилась в
// предел. Сам предел задаёт ядро и передаёт числом.

type Reply =
  | { ready: true }
  | { failed: string }
  | { id: number; reply: unknown }
  | { id: number; error: string };

interface Pending {
  id: number;
  resolve: (reply: unknown) => void;
  reject: (cause: unknown) => void;
  timer: number | undefined;
}

/** Отказы, по которым ядро узнаёт, что сборку остановил интерфейс. */
const TIMED_OUT = { kind: "TimedOut" } as const;
const CANCELLED = { kind: "Cancelled" } as const;

export interface CompileWorker {
  /** Отправить один проход компилятора воркеру. */
  post: (message: unknown) => Promise<unknown>;
  /**
   * Прекратить идущую сборку (FR-011): воркер убивается вместе с вычислением,
   * следующая сборка поднимет новый. Без идущей сборки ничего не делает —
   * воркер и его кэш остаются.
   */
  cancel: () => void;
}

/**
 * Владелец воркера. Сам воркер создаётся при первой сборке, а
 * не при открытии vault: пока предпросмотр не понадобился, второй экземпляр
 * ядра не загружается.
 */
export function createCompileWorker(limitMs: number): CompileWorker {
  let worker: Worker | null = null;
  let ready: Promise<void> | null = null;
  let pending: Pending | null = null;
  let nextId = 0;

  const drop = (): void => {
    worker?.terminate();
    worker = null;
    ready = null;
  };

  const fail = (cause: unknown): void => {
    const job = pending;
    pending = null;
    if (job !== null) {
      window.clearTimeout(job.timer);
      job.reject(cause);
    }
  };

  const start = (): Promise<void> => {
    if (ready !== null) return ready;
    const created = new Worker(new URL("./compile-worker.js", import.meta.url), {
      type: "module",
    });
    worker = created;
    ready = new Promise<void>((resolve, reject) => {
      created.onmessage = (event: MessageEvent<Reply>) => {
        const data = event.data;
        if ("ready" in data) {
          resolve();
        } else if ("failed" in data) {
          reject(new Error(data.failed));
        } else if (pending !== null && data.id === pending.id) {
          const job = pending;
          pending = null;
          window.clearTimeout(job.timer);
          if ("error" in data) job.reject(new Error(data.error));
          else job.resolve(data.reply);
        }
      };
      created.onerror = (event) => {
        const cause = new Error(event.message || "compile worker failed");
        drop();
        reject(cause);
        fail(cause);
      };
    });
    // Воркер, который не поднялся, не должен остаться «готовящимся» навсегда:
    // следующая сборка попробует создать его заново.
    ready.catch(() => drop());
    return ready;
  };

  const cancel = (): void => {
    if (pending === null) return;
    drop();
    fail(CANCELLED);
  };

  const post = async (message: unknown): Promise<unknown> => {
    await start();
    const target = worker;
    if (target === null) throw new Error("compile worker is gone");
    return new Promise<unknown>((resolve, reject) => {
      const id = ++nextId;
      // Отсчёт — с отправки сборки, а не с создания воркера: загрузка модуля в
      // предел не входит.
      const timer = window.setTimeout(() => {
        if (pending?.id !== id) return;
        pending = null;
        // Настоящая отмена: вычисление умирает вместе с воркером, а
        // следующая сборка поднимет новый.
        drop();
        reject(TIMED_OUT);
      }, limitMs);
      pending = { id, resolve, reject, timer };
      target.postMessage({ id, message });
    });
  };

  return { post, cancel };
}
