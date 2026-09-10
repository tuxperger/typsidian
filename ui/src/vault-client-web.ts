import init, { PreviewEngine, WebBackend } from "./wasm-core/typsidian_core.js";
import type { VaultClient } from "./vault-client.js";
import type { PreviewClient } from "./preview-client.js";
import { createWasmCallQueue } from "./wasm-call-queue.js";
import { classifyOpenFailure, type VaultOpenResult } from "./vault-tree/useVault.js";
import { basename } from "./paths.js";

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

/**
 * Оборачивает готовый `WebBackend` в оба интерфейса сразу.
 *
 * Вместе, а не по отдельности: предпросмотр читает связанные файлы через тот
 * же бэкенд и обязан ходить через ту же очередь. Две независимые фабрики
 * означали бы две очереди — то есть отсутствие очереди.
 */
export function webClients(
  backend: WebBackend,
  engine: PreviewEngine,
): { vault: VaultClient; preview: PreviewClient } {
  const queue = createWasmCallQueue();
  return {
    vault: webVaultClient(backend, queue),
    // Снимок несохранённых правок предпросмотру обновлять не нужно: сборка
    // ничего в vault не пишет, поэтому здесь обычный `run`, а не обёртка
    // `withSnapshotRefresh` из клиента vault.
    preview: webPreviewClient(engine, backend, (op) => queue.run(op)),
  };
}

/**
 * Оборачивает готовый `WebBackend` в общий интерфейс.
 *
 * Очередь можно передать снаружи — так предпросмотр делит её с vault
 * (см. `webClients`). Без аргумента заводит свою.
 */
export function webVaultClient(
  backend: WebBackend,
  queue = createWasmCallQueue(),
): VaultClient {
  // Ядро живёт в том же процессе одним Rust-объектом, и обращаться к нему можно
  // только по одному вызову за раз — см. `wasm-call-queue.ts`. Через очередь
  // проходят все вызовы **экземпляра**; статические методы `WebBackend`
  // (`fileKind`, `embedsInText`, `planEmbed`, …) его не заимствуют и в
  // сериализации не нуждаются.

  /**
   * Последний ответ ядра на «что будет потеряно при закрытии». Нужен ровно для
   * синхронного `hasUnresolvedEdits`: пока ядро занято, спросить его нельзя, а
   * `beforeunload` промиса не дождётся.
   */
  let unresolvedSnapshot: string[] = [];

  const withSnapshotRefresh =
    <T>(op: () => T | Promise<T>) =>
    async (): Promise<T> => {
      try {
        return await op();
      } finally {
        // `op` завершилась, заимствование отпущено, а очередь ещё никого не
        // пустила — единственный момент, когда снимок можно обновить.
        unresolvedSnapshot = backend.unresolvedEditPaths();
      }
    };

  const call = <T>(op: () => T | Promise<T>): Promise<T> =>
    queue.run(withSnapshotRefresh(op));
  const callInternal = <T>(op: () => T | Promise<T>): Promise<T> =>
    queue.runInternal(withSnapshotRefresh(op));

  return {
    // Геттер ядра, а не имя из `showDirectoryPicker`: корень знает бэкенд.
    rootName: basename(backend.rootPath()),

    list: () => call(() => backend.list()),
    read: (path) => call(() => backend.read(path)),
    write: (path, content, expected) =>
      call(() => backend.write(path, content, expected)),
    create: (path) => call(() => backend.create(path)),
    rename: (from, to) => call(() => backend.rename(from, to)),
    delete: (path) => call(() => backend.delete(path)),
    backlinks: (path) => call(() => backend.backlinks(path)),
    brokenLinks: (path) => call(() => backend.brokenLinks(path)),
    accessState: () => call(() => backend.accessState()),
    search: (query) => call(() => backend.search(query)),
    fileKind: async (path) => WebBackend.fileKind(path),

    // Берётся у ядра, а не пишется здесь литералом «ReadWrite»: значение,
    // объявляемое пользователю, и значение, применяемое гейтом записи, —
    // одно поле `WebBackend` (FR-034, FR-035).
    //
    // Через очередь не идёт и не нуждается в ней: геттер читается один раз,
    // синхронно, в момент сборки клиента — когда ни один вызов ядра ещё не в
    // полёте, — а не при каждом обращении UI.
    boardWriteProfile: backend.boardWriteProfile,

    beginEdit: (path, buffer) => call(() => backend.beginEdit(path, buffer)),
    pendingConflict: (path) => call(() => backend.pendingConflict(path)),
    conflictDiff: (path) => call(() => backend.conflictDiff(path)),
    resolveConflictKeepLocal: (path) =>
      call(() => backend.resolveConflictKeepLocal(path)),
    resolveConflictAcceptExternal: (path) =>
      call(() => backend.resolveConflictAcceptExternal(path)),
    unresolvedEditPaths: () => call(() => backend.unresolvedEditPaths()),

    hasUnresolvedEdits: () => {
      // Ядро свободно — спрашиваем его: ответ всегда точен.
      if (!queue.isBusy()) return backend.unresolvedEditPaths().length > 0;
      // Ядро занято, а `beforeunload` ответа ждать не станет. Ошибаемся в
      // сторону «есть несохранённое», как требует контракт `VaultClient`:
      // незавершённый вызов пользователя мог как раз завести правку, которой в
      // снимке ещё нет. Тик опроса правок не заводит — под ним снимка
      // достаточно.
      return queue.hasPendingUserCalls() || unresolvedSnapshot.length > 0;
    },

    subscribe: (onEvents) => {
      let cancelled = false;
      // Очередь не даёт тику наложиться на чужой вызов, но не мешает тикам
      // копиться в ней самой, если сверка идёт дольше интервала.
      let inFlight = false;

      const tick = async (): Promise<void> => {
        if (inFlight || cancelled) return;
        inFlight = true;
        try {
          const events = await callInternal(async () => {
            // Порядок из двух шагов обязателен и продиктован ядром:
            // `pollEvents` сам ничего не обнаруживает — очередь наполняет
            // `pollForChanges` (doc-комментарий `WebBackend::poll_for_changes`).
            // Без первого шага ни одно внешнее изменение не было бы замечено.
            // Оба шага идут одной операцией очереди намеренно: разорвать их
            // значило бы отдать наполненную очередь событий чужому вызову.
            await backend.pollForChanges();
            return cancelled ? [] : backend.pollEvents();
          });
          if (!cancelled && events.length > 0) onEvents(events);
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

    createBoard: (path) => call(() => backend.createBoard(path)),
    readBoard: (path) => call(() => backend.readBoard(path)),
    writeBoard: (path, content, expected) =>
      call(() => backend.writeBoard(path, content, expected)),
    writeSnapshot: (snapshotPath, svg, boardPath, boardContent) =>
      call(() =>
        backend.writeSnapshot(snapshotPath, svg, boardPath, boardContent),
      ),
    embedsOf: (notePath, noteText) =>
      call(() => backend.embedsOf(notePath, noteText)),
    embedsInText: async (notePath, noteText) =>
      WebBackend.embedsInText(notePath, noteText),
    embedsChanged: async (notePath, noteText, changed) =>
      WebBackend.embedsChanged(notePath, noteText, changed),
    planEmbed: async (notePath, noteText, boardPath, overrides) =>
      WebBackend.planEmbed(notePath, noteText, boardPath, overrides),
    embedMarkup: async (plan) => WebBackend.embedMarkup(plan),

    // ── Шаблоны и сниппеты (фича 003) ─────────────────────────────────
    //
    // Через очередь идёт всё, что заимствует backend: и чтение библиотеки, и
    // превью (оно держит кэш полем самого backend). Планирование вставки и
    // разбор события — статические функции ядра, им очередь не нужна.
    templateLibrary: () => call(() => backend.templateLibrary()),
    readTemplate: (path) => call(() => backend.readTemplate(path)),
    templatePreview: (template) => call(() => backend.templatePreview(template)),
    checkTemplateName: (name) => call(() => backend.checkTemplateName(name)),
    saveAsTemplate: (name, content, confirmOverwrite) =>
      call(() => backend.saveAsTemplate(name, content, confirmOverwrite)),
    planInsertion: async (textLength, point, content) =>
      WebBackend.planInsertion(textLength, point, content),
    templateEvent: async (event) => WebBackend.templateEvent(event),

    // ── Настройки уровня vault и разделы (фича 006) ───────────────────
    //
    // Идут через `call`, а не как статические функции выше: они читают и
    // пишут файл внутри vault, то есть заимствуют backend.
    vaultSettings: () => call(() => backend.vaultSettings()),
    setVaultSettings: (overrides) =>
      call(() => backend.setVaultSettings(overrides)),
    markSection: (folder) => call(() => backend.markSection(folder)),
    unmarkSection: (folder) => call(() => backend.unmarkSection(folder)),
  };
}

/**
 * Компилятор предпросмотра для web (фича 005).
 *
 * **Очередь теперь обязательна.** Раньше её здесь не было: движок был отдельным
 * объектом wasm-bindgen со своим заимствованием и до vault не дотягивался. С
 * появлением фазы резолва (T014-T015) сборка читает связанные файлы через
 * `WebBackend`, то есть держит его заимствование на всё время компиляции —
 * параллельный тик опроса упал бы «recursive use of an object».
 *
 * Чего здесь пока нет: сборка идёт в потоке UI. Для короткого документа это
 * незаметно, но FR-007 в полном объёме закрывает только вынос в Web Worker
 * (задача T022) — до неё тяжёлый документ подморозит ввод.
 */
function webPreviewClient(
  engine: PreviewEngine,
  backend: WebBackend,
  call: <T>(op: () => T | Promise<T>) => Promise<T>,
): PreviewClient {
  return {
    compile: (notePath, text, revision) =>
      call(() => engine.compile(backend, notePath, text, BigInt(revision))),
    // Фича 007: тот же движок, но недостающие шаблоны доустанавливаются.
    // Идёт через ту же очередь: путь пишет в vault, и наложение на цикл
    // опроса упало бы «recursive use of an object».
    compileWithTemplates: (notePath, text, revision, overrides) =>
      call(() =>
        engine.compileWithTemplates(
          backend,
          notePath,
          text,
          BigInt(revision),
          overrides,
        ),
      ),
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
    const { vault, preview } = webClients(backend, new PreviewEngine());
    return { ok: true, client: vault, preview };
  } catch (cause) {
    return { ok: false, error: classifyOpenFailure(cause) };
  }
}
