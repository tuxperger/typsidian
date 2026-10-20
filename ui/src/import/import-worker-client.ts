// Владелец Web Worker импорта (фича 021, T026). Окно шлёт запросы и ждёт
// ответы; что они значат, знает ядро в воркере.

type Reply = { id: number; value: unknown } | { id: number; error: unknown };

export interface ImportWorker {
  call<T>(request: Record<string, unknown>): Promise<T>;
  terminate(): void;
}

export function createImportWorker(): ImportWorker {
  const worker = new Worker(new URL("./import-worker.js", import.meta.url), { type: "module" });
  const pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: unknown) => void }>();
  let nextId = 0;
  worker.onmessage = (event: MessageEvent<Reply>) => {
    const reply = event.data;
    const waiter = pending.get(reply.id);
    if (waiter === undefined) return;
    pending.delete(reply.id);
    if ("error" in reply) waiter.reject(reply.error);
    else waiter.resolve(reply.value);
  };
  worker.onerror = (event) => {
    const error = { kind: "Io", message: event.message || "import worker failed" };
    for (const waiter of pending.values()) waiter.reject(error);
    pending.clear();
  };
  return {
    call: <T>(request: Record<string, unknown>): Promise<T> =>
      new Promise<T>((resolve, reject) => {
        const id = ++nextId;
        pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
        worker.postMessage({ ...request, id });
      }),
    terminate: () => {
      worker.terminate();
      for (const waiter of pending.values()) waiter.reject({ kind: "NoSource" });
      pending.clear();
    },
  };
}
