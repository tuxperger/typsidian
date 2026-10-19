import {
  attachmentsBatchCancel,
  attachmentsBatchConfirm,
  attachmentsBatchNext,
  attachmentsBatchSourceFailed,
  attachmentsBatchStart,
  attachmentsBatchStore,
  attachmentsBatchView,
  attachmentsPlanMarkup,
  attachmentsValidateFolder,
  buildTimeLimitMs,
  captureEntry,
  captureSettings,
  captureTargetPath,
  dailyDateOf,
  dailyPath,
  exportBatchCancel,
  exportBatchNext,
  exportBatchRecord,
  exportBatchSkip,
  exportBatchStart,
  exportBatchView,
  exportCandidates,
  importBegin,
  importFinish,
  ImportSource,
  importStep,
  importSurvey,
  linksBacklinksPage,
  linksLinkMention,
  linksMentionsAvailability,
  linksMentionsCancel,
  linksMentionsIn,
  linksMentionsStart,
  linksMentionsStep,
  noteOutline,
  noteOutlineActive,
  noteOutlineCarry,
  noteOutlineView,
  PreviewEngine,
  previewLocateHeading,
  propertiesEdit,
  propertiesRead,
  searchParseConditions,
  selectBlocks,
  selectImportEdits,
  selectLibrary,
  slidesCaret,
  slidesCarry,
  slidesKeymap,
  slidesLabel,
  slidesNavigate,
  slidesStart,
  SyntaxSession,
  WebBackend,
  workspaceApply,
  workspaceEmpty,
  workspaceRestore,
  workspaceSnapshot,
  workspaceView,
  type VaultRecord,
  type VaultEvent,
} from "./wasm-core/typsidian_core.js";
import { createFlushRegistry, type VaultClient } from "./vault-client.js";
import type { PreviewClient } from "./preview-client.js";
import { createWasmCallQueue, type WasmCallQueue } from "./wasm-call-queue.js";
import { ensureWasmInitialized } from "./wasm-init.js";
import { createCompileWorker } from "./preview/compile-worker-client.js";
import type { SyncClient, VaultCloner } from "./sync/sync-client.js";
import { webSyncClient, webVaultCloner } from "./sync/sync-client-web.js";
import { deviceStore, deviceStoreQueue } from "./settings/deviceStore.js";
import { changeOf } from "./settings/useSettings.js";
import {
  classifyOpenFailure,
  type LastVault,
  type VaultOpenResult,
} from "./vault-tree/useVault.js";
import { basename } from "./paths.js";
import { handleImportSource, type ImportSourcePicker } from "./import/import-source.js";

// Реализация `VaultClient` для web: ядро в том же процессе, WASM
// (`packages/core` → `wasm-pack --target web`).
//
// Единственное место в проекте, где `WebBackend` импортируется **значением**, а
// не типом. Остальной общий UI работает через `VaultClient`, поэтому оболочка
// рабочего стола не втягивает WASM в свою сборку: она этот модуль не импортирует
// вовсе, а `import type` из `wasm-core` стирается компилятором.

/** Заметок за один шаг пересчёта свойств (как `REINDEX_BATCH` ядра). */
const PROPERTIES_BATCH = 50;

/**
 * Интервал сверки с диском. У File System Access API нет подписки на изменения
 * (research.md §3), поэтому web опрашивает ядро периодически; значение выбрано
 * под бюджет SC-004 «внешнее изменение видно ≤2 с»: один тик на обнаружение
 * плюс запас на сам обход.
 */
export const VAULT_POLL_INTERVAL_MS = 1000;

/**
 * Сколько один шаг сверки держит очередь вызовов wasm (001 T136): вызов
 * пользователя ждёт опроса не дольше этого, а не всего обхода vault.
 */
const POLL_STEP_BUDGET_MS = 20;

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
): { vault: VaultClient; preview: PreviewClient; queue: WasmCallQueue } {
  const queue = createWasmCallQueue();
  return {
    // Очередь отдаётся наружу ради синхронизации (фича 011): её сессия тоже
    // заимствует backend и обязана стоять в той же очереди.
    queue,
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

  /** Открытая исходная папка импорта (фича 021); `null` — не открыта. */
  let importSource: ImportSource | null = null;
  const openedSource = (): ImportSource => {
    if (importSource === null) throw { kind: "NoSource" };
    return importSource;
  };

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

  const flushRegistry = createFlushRegistry(() => call(() => backend.unresolvedEditPaths()));

  return {
    // Геттер ядра, а не имя из `showDirectoryPicker`: корень знает бэкенд.
    rootName: basename(backend.rootPath()),

    list: () => call(() => backend.list()),
    read: (path) => call(() => backend.read(path)),
    write: (path, content, expected) =>
      call(() => backend.write(path, content, expected)),
    create: (path) => call(() => backend.create(path)),
    rename: (from, to) => call(() => backend.rename(from, to)),
    moveTargets: (from, query) => call(() => backend.moveTargets(from, query)),
    moveImpact: (request) => call(() => backend.moveImpact(request)),
    moveEntry: (request, control) => {
      // Вызов ядра занят до конца переноса, поэтому отмена — флаг, который
      // ядро читает между файлами, а не второй вызов (023 research.md §6).
      const cancel = { cancelled: control?.signal?.aborted ?? false };
      control?.signal?.addEventListener("abort", () => {
        cancel.cancelled = true;
      });
      return call(() => backend.moveEntry(request, control?.onProgress, cancel));
    },
    delete: (path) => call(() => backend.delete(path)),
    backlinks: (path) => call(() => backend.backlinks(path)),
    graph: (request) => call(() => backend.vaultGraph(request)),
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
    discardEdit: (path) => call(() => backend.discardEdit(path)),
    pendingConflict: (path) => call(() => backend.pendingConflict(path)),
    conflictDiff: (path) => call(() => backend.conflictDiff(path)),
    resolveConflictKeepLocal: (path) =>
      call(() => backend.resolveConflictKeepLocal(path)),
    resolveConflictAcceptExternal: (path) =>
      call(() => backend.resolveConflictAcceptExternal(path)),
    unresolvedEditPaths: () => call(() => backend.unresolvedEditPaths()),
    ...flushRegistry,

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
          // Цикл сверки идёт шагами, каждый — отдельной операцией очереди:
          // на большом vault цельный цикл держал очередь секундами, и вызовы
          // пользователя ждали его (001 T136). Между шагами они встают вперёд.
          let events: VaultEvent[] | null = null;
          while (events === null && !cancelled) {
            events = await callInternal(async () => {
              // `pollEvents` сам ничего не обнаруживает — очередь событий
              // наполняет завершённый цикл (doc-комментарий
              // `WebBackend::poll_step`). Последний шаг и выборка событий идут
              // одной операцией намеренно: разорвать их значило бы отдать
              // наполненную очередь событий чужому вызову.
              const finished = await backend.pollStep(POLL_STEP_BUDGET_MS);
              if (!finished) return null;
              return cancelled ? [] : backend.pollEvents();
            });
          }
          if (!cancelled && events !== null && events.length > 0) onEvents(events);
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
    // Фича 008: чистые функции ядра — статические, без очереди вызовов.
    elementSchema: async (id) => WebBackend.elementSchema(id),
    elementSchemaFor: async (template) => WebBackend.elementSchemaFor(template),
    elementAt: async (text, offset) => WebBackend.elementAt(text, offset),
    // Фича 015: свободные функции ядра — без очереди вызовов, как `elementAt`.
    outline: async (text) => noteOutline(text),
    outlineView: async (outline, request) => noteOutlineView(outline, request),
    outlineActive: async (outline, request) => noteOutlineActive(outline, request) ?? null,
    outlineCarry: async (prev, next, collapsed) => noteOutlineCarry(prev, next, collapsed),
    // Фича 024: объект ядра на редактор. Своя память, не `WebBackend`, поэтому
    // очередь вызовов wasm его не ждёт.
    openHighlight: (text) => {
      let session: SyntaxSession | null = new SyntaxSession(text);
      return {
        update: async (edits, length, windows) => session?.update(edits, length, windows),
        updateNow: (edits, length, windows) => session?.update(edits, length, windows),
        close: () => {
          session?.free();
          session = null;
        },
      };
    },
    renderElement: async (request) => WebBackend.renderElement(request),
    planElementEdit: async (request) => WebBackend.planElementEdit(request),
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

    // ── Сочетания клавиш ──────────────────────────────────────────────
    // `keybindings` — статическая: она ничего не читает с диска, это разбор
    // уже полученных настроек. Две другие пишут файл, поэтому через `call`.
    keybindings: async (overrides, mac) => WebBackend.keybindings(overrides, mac),
    setKeybinding: (command, shortcut) =>
      call(() => backend.setKeybinding(command, shortcut)),
    resetKeybinding: (command) => call(() => backend.resetKeybinding(command)),

    // ── Вкладки и разделение редактора (фича 014) ─────────────────────
    // Переход и показ берут факты из индекса vault — заимствуют backend,
    // поэтому идут через `call`. Пустое состояние и снимок — чистые функции.
    workspaceEmpty: async () => workspaceEmpty(),
    workspaceApply: (state, action, widthPx) =>
      call(() => workspaceApply(backend, state, action, widthPx)),
    workspaceView: (state, widthPx) => call(() => workspaceView(backend, state, widthPx)),
    workspaceSnapshot: async (state) => workspaceSnapshot(state),
    workspaceRestore: (saved) => call(() => workspaceRestore(backend, saved)),

    // ── Связи заметки (фича 013) ──────────────────────────────────────
    // Чтение vault идёт через `call`: страница и проход заимствуют backend.
    // Доступность и отмена прохода — чистые функции ядра без backend.
    links: {
      backlinksPage: (target, openTexts, offset, limit) =>
        call(() => linksBacklinksPage(backend, target, openTexts, offset, limit)),
      mentionsAvailability: async (target) => linksMentionsAvailability(target),
      mentionsStart: (target) => call(() => linksMentionsStart(backend, target)),
      mentionsStep: (state, openTexts) =>
        call(() => linksMentionsStep(backend, state, openTexts)),
      mentionsCancel: async (state) => linksMentionsCancel(state),
      mentionsIn: (target, paths, openTexts) =>
        call(() => linksMentionsIn(backend, target, paths, openTexts)),
      linkMention: (request, editorText) =>
        call(() => linksLinkMention(backend, request, editorText)),
    },

    // ── Показ слайдов (фича 019) ──────────────────────────────────────
    // Чистые функции ядра без backend: очередь вызовов им не нужна.
    slides: {
      start: async (slides, caret, nowMs) => slidesStart(slides, caret, nowMs),
      navigate: async (state, slides, action) => slidesNavigate(state, slides, action),
      carry: async (state, slides) => slidesCarry(state, slides),
      caret: async (slides, page) => slidesCaret(slides, page),
      label: async (slides, page) => slidesLabel(slides, page),
      keymap: async () => slidesKeymap(),
    },

    // ── Свойства заметок и выборки (фича 020) ─────────────────────────
    // Разбор и правка записи — чистые функции; отбор, индекс и библиотека
    // выборки читают и пишут vault, то есть заимствуют backend.
    properties: {
      read: async (text) => propertiesRead(text),
      edit: async (text, change) => propertiesEdit(text, change),
      selectBlocks: async (text) => selectBlocks(text),
      selectImportEdits: async (text) => selectImportEdits(text),
      selectLibrary: async () => selectLibrary(),
      parseConditions: async (text, knownKeys) => searchParseConditions(text, knownKeys),
      select: (selection) => call(() => backend.vaultSelect(selection)),
      keys: () => call(() => backend.vaultPropertyKeys()),
      current: () => call(() => backend.vaultPropertiesCurrent()),
      // Цикл по шагам ядра ведёт клиент: очередь вызовов wasm освобождается
      // между шагами, и остальные команды не ждут весь пересчёт (принцип X).
      reindex: async (control) => {
        let from = 0;
        let total = 0;
        for (;;) {
          if (control?.signal?.aborted === true) return { done: from, total, cancelled: true };
          const step = await call(() =>
            backend.vaultReindexPropertiesStep(from, PROPERTIES_BATCH),
          );
          control?.onProgress?.(step.done, step.total);
          if (step.finished) return { done: step.done, total: step.total, cancelled: false };
          from = step.done;
          total = step.total;
        }
      },
      ensureSelectLibrary: () => call(() => backend.vaultEnsureSelectLibrary()),
      selectDigest: (text) => call(() => backend.vaultSelectDigest(text)),
    },

    // ── Быстрый захват и ежедневные заметки (фича 022) ────────────────
    // Чистые функции ядра без backend; запись, «Сегодня» и соседние дни читают
    // и пишут vault, то есть заимствуют backend.
    capture: {
      settings: async (overrides) => captureSettings(overrides),
      entry: async (settings, capture, imagePath) =>
        captureEntry(settings, capture, imagePath ?? null),
      targetPath: async (settings, capture) => captureTargetPath(settings, capture),
      dailyPath: async (settings, date) => dailyPath(settings, date),
      dailyDateOf: async (settings, path) => dailyDateOf(settings, path),
      write: (capture, openBuffer, image) =>
        call(() => backend.captureWrite(capture, openBuffer ?? null, image ?? null)),
      openDaily: (date) => call(() => backend.openDaily(date)),
      dailyNeighbours: (date) => call(() => backend.dailyNeighbours(date)),
    },

    // ── Вложения (фича 016) ───────────────────────────────────────────
    // Начало и запись трогают vault — через очередь; чистые переходы пачки —
    // мимо неё, иначе отмена ждала бы записи чужого вызова.
    attachments: {
      start: (overrides, notePath, items, moment) =>
        call(() => attachmentsBatchStart(backend, overrides, notePath, items, moment)),
      next: async (state) => attachmentsBatchNext(state),
      confirm: async (state, index, accept) => attachmentsBatchConfirm(state, index, accept),
      store: (state, index, bytes) =>
        call(() => attachmentsBatchStore(backend, state, index, bytes)),
      sourceFailed: async (state, index) => attachmentsBatchSourceFailed(state, index),
      cancel: async (state) => attachmentsBatchCancel(state),
      view: async (state) => attachmentsBatchView(state),
      planMarkup: async (textLength, point, before, after, paths, block) =>
        attachmentsPlanMarkup(textLength, point, before, after, paths, block),
      validateFolder: async (raw) => attachmentsValidateFolder(raw),
    },

    // ── Импорт из Obsidian (фича 021) ─────────────────────────────────
    // Источник — отдельный объект ядра (`ImportSource`, только чтение); шаг
    // заимствует его вместе с backend, поэтому идёт через `call`. Между шагами
    // очередь свободна: правка другой заметки не ждёт весь импорт (принцип X).
    import: {
      open: async (target, name) => {
        if (target.kind !== "handle") throw { kind: "NoSource" };
        importSource?.free();
        importSource = ImportSource.fromHandle(name, target.handle);
      },
      close: async () => {
        importSource?.free();
        importSource = null;
      },
      survey: (job) => call(() => importSurvey(openedSource(), backend, job)),
      begin: (job) => call(() => importBegin(openedSource(), backend, job)),
      step: (state) => call(() => importStep(openedSource(), backend, state)),
      finish: (state) => call(() => importFinish(backend, state)),
    },
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
  // Сборка предпросмотра — в Web Worker (005 FR-007): поток интерфейса не
  // ждёт компилятор, а сборка дольше предела обрывается. Без `Worker`
  // (старый браузер, тестовая среда) ядро собирает на месте, как раньше.
  const worker =
    typeof Worker === "undefined" ? null : createCompileWorker(buildTimeLimitMs());
  if (worker !== null) engine.setCompileWorker(worker.post);
  return {
    // Мимо очереди: отмена обязана дойти, пока сборка её занимает.
    cancel: () => worker?.cancel(),
    dependencyMark: (paths) => call(() => engine.dependencyMark(backend, paths)),
    compile: (notePath, text, revision, anchor) =>
      call(() => engine.compile(backend, notePath, text, BigInt(revision), anchor)),
    // Фича 007: тот же движок, но недостающие шаблоны доустанавливаются.
    // Идёт через ту же очередь: путь пишет в vault, и наложение на цикл
    // опроса упало бы «recursive use of an object».
    compileWithTemplates: (notePath, text, revision, overrides, anchor) =>
      call(() =>
        engine.compileWithTemplates(
          backend,
          notePath,
          text,
          BigInt(revision),
          overrides,
          anchor,
        ),
      ),
    // FR-010: явная установка идёт через ту же очередь — она тоже пишет в
    // vault, и наложение на цикл опроса упало бы «recursive use of an object».
    installTemplate: (spec, overrides) =>
      call(() => engine.installTemplate(backend, spec, overrides)),
    installedTemplates: () => call(() => engine.installedTemplates(backend)),
    // Фича 009. Экспорт собирает документ и пишет файл — через очередь, как
    // `compile`: сборка держит заимствование `WebBackend`.
    exportPdf: (notePath, text, overrides) =>
      call(() => engine.exportPdf(backend, notePath, text, overrides)),
    exportCandidates: (overrides) =>
      call(() => exportCandidates(backend, overrides)),
    // Фича 015: свободная функция, vault не трогает — мимо очереди.
    locateHeading: async (runs, outline, key) => previewLocateHeading(runs, outline, key) ?? null,
    // Переходы пакета не трогают vault — вызываются напрямую, мимо очереди:
    // иначе отмена ждала бы окончания сборки текущей заметки в очереди.
    exportBatch: {
      start: async (paths, stopOnError, overrides) =>
        exportBatchStart(paths, stopOnError, overrides),
      next: async (state) => exportBatchNext(state),
      record: async (state, report, elapsedMs) =>
        exportBatchRecord(state, report, Math.round(elapsedMs)),
      skip: async (state, notePath) => exportBatchSkip(state, notePath),
      cancel: async (state) => exportBatchCancel(state),
      view: async (state) => exportBatchView(state),
    },
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
    return await openWebVaultFromHandle(handle, null);
  } catch (cause) {
    return { ok: false, error: classifyOpenFailure(cause) };
  }
}

/**
 * Открыть vault по уже полученному хендлу папки.
 *
 * `knownId` — идентификатор записи, если vault открывают из списка известных;
 * `null` — папку только что выбрали, и запись заводится здесь.
 */
async function openWebVaultFromHandle(
  handle: FileSystemDirectoryHandle,
  knownId: string | null,
): Promise<VaultOpenResult> {
  const backend = await WebBackend.fromHandle(handle.name, handle);
  const { vault, preview, queue } = webClients(backend, new PreviewEngine());
  // FR-007: открытый vault становится последним. После успеха, а не до:
  // папка, которую не удалось открыть, последней не становится.
  const remembered =
    knownId === null ? await rememberWebVault(handle) : { id: knownId, wasKnown: true };
  const vaultId = remembered?.id ?? null;
  // 006 FR-024, FR-025: утрачены ли настройки уровня vault и можно ли их
  // менять, решает ядро; отсюда ему нужен один факт — открывали ли папку
  // раньше, а знает его хранилище устройства.
  const settingsNotice = backend.serviceDirStatus(remembered?.wasKnown ?? false);
  // Синхронизации нужен идентификатор записи: под ним лежат значения
  // устройства и секреты этого vault (фича 011). Не запомнился vault или не
  // собралась сессия — vault работает без неё (FR-038).
  let sync: SyncClient | undefined;
  if (vaultId !== null) {
    try {
      sync = await webSyncClient(backend, queue, handle, vaultId);
    } catch {
      sync = undefined;
    }
  }
  return {
    ok: true,
    client: vault,
    preview,
    ...(sync === undefined ? {} : { sync }),
    ...(settingsNotice === undefined ? {} : { settingsNotice }),
  };
}

/** Жест «создать vault из удалённого репозитория» (фича 011, FR-013). */
export const cloneWebVault: VaultCloner = webVaultCloner(ensureWasmInitialized, async (handle, vaultId) => {
  try {
    return await openWebVaultFromHandle(handle, vaultId);
  } catch (cause) {
    return { ok: false, error: classifyOpenFailure(cause) };
  }
});

/**
 * Запоминает папку как последний открытый vault (FR-007).
 *
 * Хендл уходит в ядро **не разобранным**: сведение повторного добавления к уже
 * известной записи делает реестр ядра через `isSameEntry()` (FR-020 фичи 004),
 * а не этот модуль — сравнивать здесь было бы нечего, абсолютного пути в
 * браузере не существует.
 *
 * Отказ хранилища открытие не отменяет и пользователю не показывается: vault
 * уже открыт и работает, не запомнилась только точка входа следующего запуска.
 */
async function rememberWebVault(
  handle: FileSystemDirectoryHandle,
): Promise<{ id: string; wasKnown: boolean } | null> {
  try {
    const store = await deviceStore();
    return await deviceStoreQueue.run(async () => {
      // «Открывали раньше» видно только здесь: запись о той же папке не
      // дублируется, так что неизменное число записей и значит «уже была».
      const before = store.settings().known_vaults?.length ?? 0;
      const id = await store.addVault(handle, handle.name);
      const wasKnown = (store.settings().known_vaults?.length ?? 0) === before;
      await store.apply(changeOf({ current_vault: id }));
      return { id, wasKnown };
    });
  } catch {
    // Молча: действия от пользователя это не требует.
    return null;
  }
}

/**
 * Последний открытый vault по данным хранилища устройства (FR-007).
 *
 * `null` — предлагать нечего: ни одного vault ещё не открывали, либо ссылки на
 * папку больше нет. Перенесённая папка не разыскивается — это решение ядра
 * (FR-023 фичи 004).
 *
 * Разрешение на папку спрашивается **в момент открытия**, а не здесь: браузер
 * выдаёт его только в обработчике жеста пользователя, и запрос на загрузке
 * страницы обернулся бы отказом вместо заметок.
 */
export async function lastWebVault(): Promise<LastVault | null> {
  try {
    const store = await deviceStore();
    return await deviceStoreQueue.run(async () => {
      const id = store.settings().current_vault;
      if (id === undefined) return null;
      const resolved = await resolveWebVault(store, id);
      if (resolved === null) return null;
      const records = (await store.knownVaults()) as VaultRecord[];
      const record = records.find((known) => known.id === id) ?? null;
      return {
        label: record?.label ?? record?.location ?? resolved.handle.name,
        // Молчаливое открытие возможно, только пока разрешение действует:
        // браузер выдаёт его заново лишь в обработчике жеста.
        readyToOpen: resolved.access === "granted",
        open: () => openRememberedWebVault(resolved.handle, id),
      };
    });
  } catch {
    return null;
  }
}

/**
 * Открыть известный vault по его записи в списке — жест переключения из панели
 * настроек (US2 фичи 004, AS3).
 *
 * Отдельная точка входа, а не `lastWebVault`: переключение идёт на **выбранную**
 * запись, и текущей она становится в ядре, а не здесь.
 */
export async function openWebVaultById(id: string): Promise<VaultOpenResult> {
  let handle: FileSystemDirectoryHandle;
  try {
    const store = await deviceStore();
    const resolved = await deviceStoreQueue.run(() => resolveWebVault(store, id));
    // Папки больше нет: та же причина, что показывает список как недоступную
    // запись (FR-023).
    if (resolved === null) return { ok: false, error: { kind: "FolderNotFound" } };
    handle = resolved.handle;
  } catch (cause) {
    return { ok: false, error: classifyOpenFailure(cause) };
  }
  return openRememberedWebVault(handle, id);
}

/** Ссылка на папку записи и состояние доступа к ней — ответ `resolveVault`. */
interface ResolvedVault {
  access: "granted" | "needsPermission";
  handle: FileSystemDirectoryHandle;
}

/**
 * Разбирает ответ `WebStore.resolveVault`.
 *
 * Типизация на границе wasm-bindgen: объект приходит нетипизированным, и
 * сужение здесь — единственное место, где о природе хендла вообще говорится.
 * Ядро его не разбирает и сравнивает только через `isSameEntry()` (FR-020).
 *
 * Вызывать **внутри** операции очереди: метод заимствует объект ядра.
 */
async function resolveWebVault(
  store: Awaited<ReturnType<typeof deviceStore>>,
  id: string,
): Promise<ResolvedVault | null> {
  const answer: { access?: unknown; handle?: unknown } = await store.resolveVault(id);
  const handle = answer.handle;
  if (answer.access === "missing" || handle === undefined || handle === null) return null;
  return {
    access: answer.access === "granted" ? "granted" : "needsPermission",
    handle: handle as FileSystemDirectoryHandle,
  };
}

/**
 * Открывает запомненный vault, переспросив разрешение на папку.
 *
 * `requestPermission()` вызывается всегда: на уже выданном разрешении он
 * отвечает `granted`, ничего не спрашивая, а отозванное — это штатный случай, а
 * не поломка (`Resolved::NeedsPermission` в ядре).
 */
async function openRememberedWebVault(
  handle: FileSystemDirectoryHandle,
  id: string,
): Promise<VaultOpenResult> {
  try {
    await ensureWasmInitialized();
    const permission = await requestFolderPermission(handle);
    if (permission !== "granted") return { ok: false, error: { kind: "PermissionDenied" } };
    return await openWebVaultFromHandle(handle, id);
  } catch (cause) {
    return { ok: false, error: classifyOpenFailure(cause) };
  }
}

/**
 * Запрос разрешения на папку. Метода может не быть вовсе — в браузере без
 * File System Access API этого хендла не существует, но версии, где он есть, а
 * `requestPermission` ещё нет, встречаются.
 */
async function requestFolderPermission(handle: FileSystemDirectoryHandle): Promise<string> {
  const request = (
    handle as unknown as {
      requestPermission?: (options: { mode: string }) => Promise<string>;
    }
  ).requestPermission;
  if (!request) return "granted";
  return request.call(handle, { mode: "readwrite" });
}

/**
 * Выбор исходной папки импорта в браузере (фича 021, FR-001): папка открывается
 * только для чтения. Нет `showDirectoryPicker` — импорт недоступен (экран C5).
 */
export function webImportPicker(): ImportSourcePicker {
  const showPicker = (): ((options: { mode: "read" }) => Promise<FileSystemDirectoryHandle>) | undefined =>
    (
      window as unknown as {
        showDirectoryPicker?: (options: { mode: "read" }) => Promise<FileSystemDirectoryHandle>;
      }
    ).showDirectoryPicker;
  return {
    get available() {
      return typeof window !== "undefined" && showPicker() !== undefined;
    },
    pick: async () => {
      const picker = showPicker();
      if (picker === undefined) return null;
      try {
        return handleImportSource(await picker.call(window, { mode: "read" }));
      } catch (cause) {
        // Закрытый диалог — отмена, а не отказ.
        if (cause instanceof DOMException && cause.name === "AbortError") return null;
        throw cause;
      }
    },
  };
}
