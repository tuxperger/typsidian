// Web Worker импорта из Obsidian (фича 021, T026, принцип X).
//
// Второй экземпляр wasm-ядра: открывает свой `WebBackend` для vault и
// `ImportSource` для папки-источника из хендлов, присланных окном, и
// выполняет шаги импорта у себя. Перевод заметки в 1 МБ — сотни миллисекунд
// (research.md §12): в потоке интерфейса они замораживали бы ввод. Записи
// воркера окно видит своим опросом — как изменения снаружи.
import init, {
  ImportSource,
  WebBackend,
  importBegin,
  importFinish,
  importStep,
  importSurvey,
} from "../wasm-core/typsidian_core.js";

type Request =
  | {
      id: number;
      kind: "open";
      vault: { name: string; handle: FileSystemDirectoryHandle };
      source: { name: string; handle: FileSystemDirectoryHandle };
    }
  | { id: number; kind: "survey" | "begin"; job: unknown }
  | { id: number; kind: "step" | "finish"; state: unknown };

interface WorkerScope {
  onmessage: ((event: MessageEvent<Request>) => void) | null;
  postMessage(message: unknown): void;
}

const scope = self as unknown as WorkerScope;
const ready = init();

let vault: WebBackend | null = null;
let source: ImportSource | null = null;

const opened = (): { vault: WebBackend; source: ImportSource } => {
  if (vault === null || source === null) throw { kind: "NoSource" };
  return { vault, source };
};

async function handle(request: Request): Promise<unknown> {
  await ready;
  switch (request.kind) {
    case "open": {
      source?.free();
      vault?.free();
      vault = await WebBackend.fromHandle(request.vault.name, request.vault.handle);
      source = ImportSource.fromHandle(request.source.name, request.source.handle);
      return null;
    }
    case "survey": {
      const { vault, source } = opened();
      return importSurvey(source, vault, request.job as never);
    }
    case "begin": {
      const { vault, source } = opened();
      return importBegin(source, vault, request.job as never);
    }
    case "step": {
      const { vault, source } = opened();
      return importStep(source, vault, request.state as never);
    }
    case "finish": {
      const { vault } = opened();
      return importFinish(vault, request.state as never);
    }
  }
}

// Шаги идут по одному: следующий запрос ждёт предыдущий, как и в окне.
let tail: Promise<unknown> = Promise.resolve();
scope.onmessage = (event) => {
  const request = event.data;
  tail = tail.then(() =>
    handle(request).then(
      (value) => scope.postMessage({ id: request.id, value }),
      (error: unknown) =>
        scope.postMessage({
          id: request.id,
          error: error instanceof Error ? { kind: "Io", message: error.message } : error,
        }),
    ),
  );
};
