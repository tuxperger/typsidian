// Web Worker сборки предпросмотра (005 FR-007, T022).
//
// Второй экземпляр wasm-ядра: принимает готовый снимок, выполняет один проход
// компилятора и отдаёт результат. Форму сообщений знает только ядро
// (`packages/core/src/preview/worker.rs`) — здесь они не разбираются, а
// передаются как есть.
import init, { runCompileJob } from "../wasm-core/typsidian_core.js";

interface WorkerScope {
  onmessage: ((event: MessageEvent<{ id: number; message: unknown }>) => void) | null;
  postMessage(message: unknown): void;
}

const scope = self as unknown as WorkerScope;

// Модуль с компилятором поднимается один раз; о готовности воркер сообщает
// сам, чтобы предел времени сборки не тратился на его загрузку.
const ready = init().then(
  () => scope.postMessage({ ready: true }),
  (cause: unknown) => scope.postMessage({ failed: String(cause) }),
);

scope.onmessage = (event) => {
  const { id, message } = event.data;
  void ready.then(() => {
    try {
      scope.postMessage({ id, reply: runCompileJob(message) });
    } catch (cause) {
      scope.postMessage({ id, error: String(cause) });
    }
  });
};
