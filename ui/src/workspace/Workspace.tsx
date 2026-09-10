"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ConflictDialog } from "../editor/ConflictDialog.js";
import {
  describeResolveError,
  useConflictResolution,
} from "../editor/useConflictResolution.js";
import { useWindowCloseGuard } from "../editor/useWindowCloseGuard.js";
import { AccessStateBanner } from "../vault-tree/AccessStateBanner.js";
import { BulkChangeIndicator } from "../vault-tree/BulkChangeIndicator.js";
import {
  ErrorBanner,
  type VaultOperationError,
} from "../vault-tree/ErrorBanner.js";
import { OpenVault } from "../vault-tree/OpenVault.js";
import { useVault, type VaultOpener } from "../vault-tree/useVault.js";
import type { VaultClient } from "../vault-client.js";
import { useVaultSettings } from "../settings/useVaultSettings.js";

import { DEFAULT_NEW_BOARD_NAME } from "../vault-tree/VaultTreeActions.js";
import { Sidebar } from "../vault-tree/Sidebar.js";
import type {
  CreateError,
  Note,
  Template,
  VaultEvent,
} from "../wasm-core/typsidian_core.js";
import { BoardCanvas } from "../boards/BoardCanvas.js";
import { BoardConflictDialog } from "../boards/BoardConflictDialog.js";
import { useBoardConflict } from "../boards/useBoardConflict.js";
import { useBoardDocument } from "../boards/useBoardDocument.js";
import { isBoard } from "../vault-tree/fileKind.js";
import { NoteEditor } from "../editor/NoteEditor.js";
import { PreviewPane, type CompileStatus } from "../preview/PreviewPane.js";
import type { PreviewClient } from "../preview-client.js";
import { Icon, IconSprite } from "../icons/Icons.js";
import { CommandPalette, type PaletteCommand } from "./CommandPalette.js";
import { GraphView } from "../graph/GraphView.js";
import { SearchOverlay } from "../search/SearchOverlay.js";
import { TemplateGallery } from "../templates/TemplateGallery.js";
import { SaveAsTemplateDialog } from "../templates/SaveAsTemplateDialog.js";
import { useTemplateLibrary } from "../templates/useTemplateLibrary.js";
import { plainInputTarget } from "../templates/insertion/plainInputTarget.js";
import type { EditorView } from "@codemirror/view";

export interface WorkspaceProps {
  /**
   * Жест «открыть папку как vault» — единственное, что различается между
   * клиентами (001 T115): в браузере это `showDirectoryPicker`, на рабочем
   * столе — нативный диалог ОС. Всё остальное поведение общее.
   */
  openVaultFolder: VaultOpener;
  /**
   * Панель настроек (фича 004). Приходит извне, а не импортируется здесь,
   * потому что её хранилище платформенное: `SettingsPanel` работает через
   * `WebStore` (OPFS + IndexedDB), и на рабочем столе он писал бы настройки в
   * песочницу окна вместо каталога конфигурации ОС. Пока для desktop нет
   * своего хранилища, кнопка настроек там не показывается — лучше её
   * отсутствие, чем настройки, уходящие не туда.
   *
   * Принимает клиента открытого vault: с фичей 006 у панели появился раздел,
   * работающий не с хранилищем устройства, а с файлом внутри vault — политика
   * расположения файлов вставок и помеченные разделы (FR-002, FR-015).
   * Функция, а не готовый узел: клиент появляется только после открытия
   * папки, и панель обязана увидеть именно текущий.
   */
  settings?: (client: VaultClient | null) => React.ReactNode;
  /**
   * Компилятор предпросмотра (фича 005). Приходит извне по той же причине, что
   * и панель настроек: на web это wasm в том же процессе, на рабочем столе —
   * команда Tauri. Пока клиент не передан, колонка предпросмотра честно
   * сообщает, что сборка недоступна, вместо того чтобы делать вид, что она
   * есть.
   */
  preview?: PreviewClient | null;
}

/**
 * Рабочая область приложения (plan.md → Project Structure): собирает уже
 * существующие части `ui/` в одно работающее приложение и **не добавляет
 * ничего от себя** — ни правил, ни проверок, ни состояний (принцип I). Всё,
 * что здесь есть, — это композиция и передача колбэков.
 *
 * Одна и та же для web и desktop: оболочки различаются только тем, какой
 * `VaultClient` сюда попадает (WASM в том же процессе против команд Tauri
 * поверх `NativeBackend`) — 001 T112. Если бы у desktop была своя копия этой
 * композиции, «одинаковое поведение на всех таргетах» проверялось бы чтением
 * двух файлов вместо одного (принцип II).
 *
 * Каркас — из макета Claude Design «Typsidian: Typst Editor Design»
 * (`design_handoff_typsidian/README.md` → Screens § 1 Workspace): шапка 44px,
 * боковая панель 248px, дальше две гибкие колонки. В колонке предпросмотра
 * теперь настоящий свёрстанный документ (фича 005): её заполняет
 * `PreviewPane`, собирая заметку из буфера редактора через `PreviewClient`.
 */
export function Workspace({
  openVaultFolder,
  settings,
  preview = null,
}: WorkspaceProps): React.JSX.Element {
  const [openNote, setOpenNote] = useState<Note | null>(null);
  // Текст открытой заметки из буфера редактора: предпросмотр собирается из
  // него, а не из файла на диске (FR-002).
  const [editorText, setEditorText] = useState("");
  const [operationError, setOperationError] = useState<VaultOperationError | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  // Экран 2 макета — отдельный вид, а не панель: граф занимает всё окно.
  const [view, setView] = useState<"workspace" | "graph">("workspace");
  const [paletteOpen, setPaletteOpen] = useState(false);
  // Расширенный поиск (макет «Утилиты» § Поиск): ⌘⇧F и команда палитры.
  const [searchOpen, setSearchOpen] = useState(false);
  // Галерея заготовок (макет «Утилиты» § Шаблоны, фича 003): ⌘⇧T и команда
  // палитры.
  const [templatesOpen, setTemplatesOpen] = useState(false);
  // Текст, который сохраняется как заготовка. `null` — диалог закрыт.
  const [savingTemplate, setSavingTemplate] = useState<string | null>(null);
  // Доступ к живому редактору для порта вставки. Функция, а не значение:
  // `CodeEditor` пересоздаётся при смене заметки.
  const [getEditorView, setGetEditorView] = useState<
    (() => EditorView | null) | null
  >(null);
  const [compile, setCompile] = useState<CompileStatus>({
    isBuilding: false,
    lastMs: null,
  });
  const [rebuildToken, setRebuildToken] = useState(0);
  // Последнее изменение файла в vault. Передаётся вниз, а не обрабатывается
  // здесь: `pollEvents` осушает очередь ядра, поэтому второй подписчик на
  // события заметки перехватывал бы их у первого (FR-005 фичи 002).
  const [lastChange, setLastChange] = useState<{
    path: string;
    revision: number;
  } | null>(null);
  const fileChanged = useCallback((path: string) => {
    setLastChange((previous) => ({ path, revision: (previous?.revision ?? 0) + 1 }));
  }, []);

  // Счётчик изменений библиотеки заготовок (фича 003, FR-003).
  //
  // Считается **по каждому событию отдельно**, а не по последнему из пачки:
  // сохранив только последнее, мы потеряли бы событие о заготовке, за которым
  // в той же пачке пришло событие о постороннем файле.
  //
  // Затрагивает ли событие библиотеку, отвечает ядро: сверять путь с
  // «templates/» здесь значило бы завести второе место, знающее это правило
  // (принцип I).
  const [libraryRevision, setLibraryRevision] = useState(0);
  // Клиент появляется позже этого колбэка, поэтому берётся из ref: сам колбэк
  // обязан быть стабильным — он уезжает в подписку на события.
  const clientRef = useRef<VaultClient | null>(null);
  const noticeEvent = useCallback((event: VaultEvent) => {
    void (async () => {
      const effect = await clientRef.current?.templateEvent(event);
      // Оба варианта — повод перечитать состав: у `TemplateChanged` меняется
      // отпечаток заготовки, а по нему карточка пересчитывает превью.
      if (effect !== undefined) setLibraryRevision((count) => count + 1);
    })();
  }, []);

  // Конфликты приходят событиями из ядра; решение принимает пользователь в
  // диалоге, а не эта оболочка (FR-021).
  const [clientForConflicts, setClientForConflicts] = useState<
    Parameters<typeof useConflictResolution>[0]
  >(null);
  const conflicts = useConflictResolution(clientForConflicts);
  // Настройки уровня vault нужны предпросмотру: в них живёт выключатель
  // автозагрузки шаблонов и адреса, указанные пользователем (фича 007).
  const templateSettings = useVaultSettings(clientForConflicts);

  const { state, openVault, refresh } = useVault(openVaultFolder, {
    onConflictDetected: (notePath) => conflicts.noteConflicted(notePath, "diskChanged"),
    onNoteDeletedWithUnsavedEdits: (notePath) =>
      conflicts.noteConflicted(notePath, "deleted"),
    // Заметка, открытая в редакторе, могла исчезнуть снаружи: закрываем
    // панель, чтобы не показывать содержимое несуществующего файла.
    onNoteDeleted: (path) => {
      setOpenNote((prev) => (prev?.path === path ? null : prev));
      fileChanged(path);
    },
    // Изменение доски снаружи устаревает её вставки — событие о доске идёт тем
    // же потоком, что и о заметке (FR-005 фичи 002).
    onNoteUpdated: fileChanged,
    onNoteCreated: fileChanged,
    onAnyEvent: noticeEvent,
  });

  // `useConflictResolution` создаётся до того, как появится backend, поэтому
  // связываем их, как только vault открыт.
  if (state.client && clientForConflicts !== state.client) {
    setClientForConflicts(state.client);
  }
  // Тот же приём, что у `handlersRef` в `useVaultEvents`: стабильный колбэк
  // подписки читает текущего клиента, а не тот, что был при его создании.
  clientRef.current = state.client;

  // Вид открытого файла спрашивается у ядра, а не выводится из расширения
  // здесь (принцип I). `openNote` может быть и доской: дерево показывает оба
  // вида одинаковыми записями, различие несёт `FileKind`.
  const isBoardOpen = openNote !== null && isBoard(openNote);
  const board = useBoardDocument(
    isBoardOpen ? state.client : null,
    isBoardOpen && openNote ? openNote.path.toString() : null,
  );

  // FR-037: предупреждение перед закрытием окна с неразрешёнными правками.
  // Доски подчиняются тому же правилу: несохранённое на полотне удерживает
  // окно так же, как несохранённый текст (FR-011 фичи 002).
  useWindowCloseGuard(state.client);
  useEffect(() => {
    if (board.saveState.state === "Clean") return;
    const warn = (event: BeforeUnloadEvent): void => {
      event.preventDefault();
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [board.saveState]);

  // Переключение на другую запись обязано дописать доску: иначе последние
  // штрихи останутся в буфере дебаунса (research.md §6 фичи 002).
  const openEntry = useCallback(
    (next: Note) => {
      if (isBoardOpen) void board.flush();
      setOpenNote(next);
    },
    [isBoardOpen, board],
  );

  const handleAcceptExternal = useCallback(async () => {
    const external = await conflicts.acceptExternal();
    // `null` — внешней версии нет: заметку удалили снаружи (FR-025).
    if (external === null) setOpenNote(null);
  }, [conflicts]);

  // Доски vault для выбора при вставке (FR-018). Тот же список, что в дереве:
  // отдельного запроса к ядру за досками нет, вид спрашивается у ядра.
  const boards = useMemo(
    () =>
      state.notes
        .filter((note) => isBoard(note))
        .map((note) => ({ path: note.path.toString(), name: note.name })),
    [state.notes],
  );

  /**
   * Открыть файл по пути. Список берётся у ядра заново, а не из `state.notes`:
   * только что созданный файл в нём ещё не появился.
   */
  const openPath = useCallback(
    async (path: string) => {
      if (!state.client) return;
      const notes = await state.client.list();
      const found = notes.find((note) => note.path.toString() === path);
      // Файла нет в дереве — он удалён или перемещён: состояние вставки уже
      // говорит об этом (FR-031), и подменять его чем-то другим тут нечем.
      if (found) setOpenNote(found);
    },
    [state.client],
  );

  /** Переход от вставки в тексте к её доске на полотне (FR-025). */
  const openBoard = useCallback(
    (boardPath: string) => {
      if (isBoardOpen) void board.flush();
      void openPath(boardPath);
    },
    [isBoardOpen, board, openPath],
  );

  /**
   * Создать доску, когда вставлять пока нечего (US2 AS2). Создание идёт тем же
   * вызовом ядра, что и из дерева, и сразу открывает полотно: пользователь
   * просил доску, а не запись в списке.
   */
  const createBoard = useCallback(async () => {
    if (!state.client) return;
    try {
      const board = await state.client.createBoard(DEFAULT_NEW_BOARD_NAME);
      await openPath(board.path.toString());
      await refresh();
    } catch (cause) {
      setOperationError({ operation: "create", error: asCreateFailure(cause) });
    }
  }, [state.client, refresh, openPath]);

  // Конфликт у доски обнаруживается тем же механизмом, что у заметки
  // (research.md §7 фичи 002) — различается только представление: текстовый diff
  // для рисунка спека запрещает, поэтому показываются две превью.
  // Вид берётся из записи дерева: конфликт всегда о файле, который приложение
  // знает. Если записи нет — файл удалён снаружи, превью строить не из чего, и
  // такой конфликт показывается обычным диалогом заметки (FR-025).
  const boardConflictPath = useMemo(() => {
    const path = conflicts.current?.notePath ?? null;
    if (path === null) return null;
    const record = state.notes.find((note) => note.path.toString() === path);
    return record && isBoard(record) ? path : null;
  }, [conflicts, state.notes]);
  const boardVersions = useBoardConflict(state.client, boardConflictPath);

  const handleAcceptExternalBoard = useCallback(async () => {
    const external = await conflicts.acceptExternal();
    // Внешней версии нет — доску удалили снаружи: показывать полотно нечем.
    if (external === null) setOpenNote(null);
    else board.replace(external);
  }, [conflicts, board]);

  // Подпись в подвале дерева («9 заметок · 3 папки» в макете) считается по тем
  // же заметкам, что показаны в дереве: отдельного запроса к ядру за счётчиком
  // нет и быть не должно.
  const folderCount = useMemo(() => countFolders(state.notes), [state.notes]);

  // Библиотека заготовок (фича 003). Читается здесь, а не в галерее: тот же
  // состав нужен второму уровню меню «+», и два места, спрашивающие ядро о
  // библиотеке, разошлись бы между собой.
  const templateLibrary = useTemplateLibrary(state.client, libraryRevision);
  const templates = useMemo<Template[]>(
    () => templateLibrary.view?.templates ?? [],
    [templateLibrary.view],
  );

  // Заготовка, выбранная в галерее для новой заметки. Уезжает в дерево, где имя
  // и расположение задаются тем же инлайн-вводом, что и у пустой заметки
  // (FR-022) — своей формы ввода у галереи нет намеренно.
  const [createFromTemplate, setCreateFromTemplate] = useState<{
    template: Template;
    revision: number;
  } | null>(null);

  // Порт вставки (contracts/insertion-target.md). Пересоздаётся только вместе с
  // доступом к редактору: сама цель заимствует его лениво, в момент вставки.
  const insertion = useMemo(
    () => (getEditorView === null ? null : plainInputTarget(getEditorView)),
    [getEditorView],
  );

  // Стабильная ссылка обязательна: `NoteEditor` объявляет доступ к редактору из
  // эффекта, зависящего от этого колбэка. Новая функция на каждый рендер
  // заставляла бы эффект перезапускаться, снимать доступ и ставить заново —
  // то есть менять состояние на каждый рендер, а это цикл.
  const editorReady = useCallback((getView: (() => EditorView | null) | null) => {
    // Функция кладётся в состояние через обёртку: `useState` трактует
    // переданную функцию как ленивое вычисление значения, и без обёртки сюда
    // попал бы её результат, а не она сама.
    setGetEditorView(() => getView);
  }, []);

  // ⌘K открывает и закрывает палитру откуда угодно, Escape — только закрывает
  // (handoff § Screens 3). Слушатель здесь, а не в самой палитре: открыть её
  // нужно и тогда, когда её в дереве компонентов ещё нет. ⌘⇧F — расширенный
  // поиск, то же сочетание, что в таблице команд макета «Утилиты».
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const accel = event.metaKey || event.ctrlKey;
      if (accel && event.shiftKey && event.key.toLowerCase() === "f") {
        event.preventDefault();
        setSearchOpen((open) => !open);
      } else if (accel && event.shiftKey && event.key.toLowerCase() === "t") {
        // Галерея заготовок (фича 003). ⌘⇧T свободно: ⌘⇧F занят поиском,
        // ⌘⇧N — созданием доски, ⌘⇧E макет отдаёт логу экспорта, которого в
        // приложении нет.
        event.preventDefault();
        setTemplatesOpen((open) => !open);
      } else if (accel && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      } else if (event.key === "Escape") {
        setPaletteOpen(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const toggleView = useCallback(() => {
    // Уход с рабочей области обязан дописать доску: дебаунс автосохранения
    // иначе выстрелит в уже размонтированное полотно.
    if (isBoardOpen) void board.flush();
    setView((current) => (current === "workspace" ? "graph" : "workspace"));
  }, [isBoardOpen, board]);

  /**
   * Команды палитры. Только те, что действительно что-то делают: список из
   * макета содержит экспорт BibTeX и переключение vim-режима, которых в
   * приложении нет, а мёртвая строка в палитре хуже её отсутствия.
   */
  const commands = useMemo<PaletteCommand[]>(() => {
    const items: PaletteCommand[] = [
      {
        id: "graph",
        title: view === "graph" ? "Вернуться в Workspace" : "Открыть граф связей",
        icon: "network",
        run: toggleView,
      },
      {
        id: "search",
        title: "Расширенный поиск",
        icon: "search",
        meta: "⌘⇧F",
        run: () => setSearchOpen(true),
      },
      {
        id: "templates",
        title: "Шаблоны и сниппеты",
        icon: "file",
        meta: "⌘⇧T",
        run: () => setTemplatesOpen(true),
      },
      {
        id: "new-board",
        title: "Создать доску",
        icon: "image",
        run: () => void createBoard(),
      },
      {
        id: "rebuild",
        title: "Пересобрать предпросмотр",
        icon: "refresh",
        run: () => setRebuildToken((token) => token + 1),
      },
    ];
    if (settings !== undefined) {
      items.push({
        id: "settings",
        title: "Открыть настройки",
        icon: "terminal",
        run: () => setShowSettings(true),
      });
    }
    return items;
  }, [view, toggleView, createBoard, settings]);

  if (!state.client) {
    return (
      <main className="ts-welcome">
        <h1 className="ts-welcome__mark">
          <span className="ts-wordmark__slash">/</span>typsidian
        </h1>
        <p>Выберите папку с заметками — она станет vault. Файлы остаются на вашем диске.</p>
        <OpenVault isLoading={state.isLoading} error={state.error} onOpen={openVault} />
      </main>
    );
  }

  return (
    <main className="ts-app">
      {/* Определения иконок монтируются один раз на всё приложение; всё
          остальное ссылается на них через `<use>`. */}
      <IconSprite />

      <header className="ts-header">
        <div className="ts-header__group">
          <span className="ts-wordmark">
            <span className="ts-wordmark__slash">/</span>typsidian
          </span>
          <span className="ts-divider" />
          <Breadcrumb rootName={state.client.rootName} path={openNote?.path ?? null} />
        </div>
        <div className="ts-header__group">
          {/* Статус сборки — только на рабочей области: в графе собирать
              нечего, и макет его там не показывает. */}
          {view === "workspace" && (
            <div className="ts-compile">
              {compile.isBuilding ? (
                <>
                  <div className="ts-hazard" role="progressbar" aria-label="Идёт сборка">
                    <div className="ts-hazard__fill" />
                  </div>
                  <span className="ts-compile__label">компиляция…</span>
                </>
              ) : (
                <>
                  <span className="ts-compile__dot" aria-hidden="true" />
                  <span className="ts-compile__label">
                    {compile.lastMs === null ? "готово" : `готово · ${compile.lastMs} ms`}
                  </span>
                </>
              )}
              <button
                type="button"
                className="ts-btn ts-btn--icon"
                title="Пересобрать"
                aria-label="Пересобрать предпросмотр"
                disabled={compile.isBuilding}
                onClick={() => setRebuildToken((token) => token + 1)}
              >
                <Icon name="refresh" />
              </button>
            </div>
          )}

          <span className="ts-divider" />

          <button
            type="button"
            className="ts-btn"
            aria-pressed={view === "graph"}
            title="Граф связей"
            onClick={toggleView}
          >
            {view === "workspace" ? (
              <Icon name="network" size={14} />
            ) : (
              <Icon name="chevron" size={14} className="ts-icon--flip" />
            )}
            {view === "workspace" ? "Граф" : "Workspace"}
          </button>

          <button
            type="button"
            className="ts-btn ts-btn--cmd"
            title="Палитра команд"
            aria-label="Открыть палитру команд"
            onClick={() => setPaletteOpen(true)}
          >
            <Icon name="cmd" size={12} />K
          </button>

          {settings !== undefined && (
            <button
              type="button"
              className="ts-btn"
              aria-pressed={showSettings}
              onClick={() => setShowSettings((shown) => !shown)}
            >
              {showSettings ? "Закрыть настройки" : "Настройки"}
            </button>
          )}
        </div>
      </header>

      <div className="ts-notices">
        <AccessStateBanner
          accessState={state.accessState}
          onReopenVault={openVault}
          settingsLost={state.settingsLost}
          // Недоступность записи видна и без события: `accessState` уже здесь,
          // а событие из ядра приходит только при **смене** состояния — при
          // открытии уже недоступного vault его бы не было (FR-025).
          settingsReadOnly={
            state.settingsReadOnly || state.accessState === "readOnly"
          }
        />
        <BulkChangeIndicator active={state.bulkChangeDepth > 0} />
        <ErrorBanner error={operationError} onDismiss={() => setOperationError(null)} />
      </div>

      {view === "graph" ? (
        <GraphView
          client={state.client}
          notes={state.notes}
          currentPath={openNote?.path.toString() ?? null}
          onOpenNote={(note) => {
            openEntry(note);
            setView("workspace");
          }}
          onBack={toggleView}
        />
      ) : (
      <div className="ts-body">
        <Sidebar
          client={state.client}
          notes={state.notes}
          openPath={openNote?.path.toString() ?? null}
          folderCount={folderCount}
          onOpenNote={openEntry}
          onNotesChanged={refresh}
          onCreated={(path) => void openPath(path)}
          onError={setOperationError}
          onOpenGraph={toggleView}
          templates={templates}
          onOpenTemplates={() => setTemplatesOpen(true)}
          createFromTemplate={createFromTemplate}
        />

        {/* FR-006, FR-007 фичи 002: доска занимает место панели
            «редактор + preview» в том же окне, отдельного окна не открывается;
            возврат к текстовой заметке возвращает прежний режим. */}
        {isBoardOpen ? (
          <BoardCanvas
            name={openNote?.name ?? null}
            content={board.content}
            contentRevision={board.contentRevision}
            saveState={board.saveState}
            readError={board.readError}
            // FR-034: клиент без права записи доски объявляет это
            // пользователю и не даёт инструментов. Само право приходит из
            // ядра через `VaultClient`, а не выводится здесь из таргета:
            // ветвления по платформе в общем UI быть не должно (принцип II).
            viewModeEnabled={state.client.boardWriteProfile === "ViewOnly"}
            onChange={board.changed}
          />
        ) : (
          <>
            <NoteEditor
              client={state.client}
              note={openNote}
              onClose={() => setOpenNote(null)}
              boards={boards}
              externalChange={lastChange}
              onOpenBoard={openBoard}
              onCreateBoard={() => void createBoard()}
              onContentChange={setEditorText}
              onEditorReady={editorReady}
              onSaveAsTemplate={setSavingTemplate}
            />

            {/* Компилятор от открытого vault приоритетнее переданного пропом:
                он делит с ним очередь вызовов ядра, а проп остаётся для
                оболочек, которые дают предпросмотр отдельно. */}
            <PreviewPane
              client={state.preview ?? preview}
              notePath={openNote?.path ?? null}
              text={editorText}
              onStatus={setCompile}
              rebuildToken={rebuildToken}
              // Фича 007: настройки решают, идти ли за недостающим шаблоном
              // в сеть. Читаются здесь, а не в панели: панель предпросмотра
              // vault-клиентом не владеет.
              vaultOverrides={templateSettings.overrides}
            />
          </>
        )}
      </div>
      )}

      {paletteOpen && (
        <CommandPalette
          notes={state.notes}
          commands={commands}
          onOpenNote={(note) => {
            openEntry(note);
            setView("workspace");
          }}
          onClose={() => setPaletteOpen(false)}
        />
      )}

      {searchOpen && (
        <SearchOverlay
          client={state.client}
          currentNote={openNote}
          onOpenNote={(path) => {
            const note = state.notes.find((entry) => entry.path.toString() === path);
            if (note) openEntry(note);
            setView("workspace");
          }}
          onClose={() => setSearchOpen(false)}
        />
      )}

      {templatesOpen && (
        <TemplateGallery
          client={state.client}
          library={templateLibrary}
          insertion={insertion}
          onCreateNote={(template) => {
            setTemplatesOpen(false);
            // Дерево возьмёт отсюда заготовку и откроет свой инлайн-ввод имени.
            setCreateFromTemplate((previous) => ({
              template,
              revision: (previous?.revision ?? 0) + 1,
            }));
          }}
          onOpenSource={(path) => void openPath(path)}
          {...(openNote !== null && !isBoardOpen
            ? {
                onSaveCurrentNote: () => {
                  setTemplatesOpen(false);
                  setSavingTemplate(editorText);
                },
              }
            : {})}
          onClose={() => setTemplatesOpen(false)}
        />
      )}

      {savingTemplate !== null && (
        <SaveAsTemplateDialog
          client={state.client}
          content={savingTemplate}
          suggestedName={openNote?.name ?? "Заготовка"}
          onSaved={() => {
            setSavingTemplate(null);
            // Состав библиотеки перечитывается у ядра, а не дописывается здесь:
            // галерея — проекция индекса, и второй способ её обновить разошёлся
            // бы с первым.
            void templateLibrary.refresh();
            void refresh();
          }}
          onCancel={() => setSavingTemplate(null)}
        />
      )}

      {showSettings && (
        <div className="ts-overlay">
          <div className="ts-sheet">
            <div className="ts-sheet__head">
              <span className="ts-eyebrow">Настройки</span>
              <span className="ts-spacer" />
              <button
                type="button"
                className="ts-btn"
                onClick={() => setShowSettings(false)}
              >
                Закрыть
              </button>
            </div>
            {/* Без внутренних отступов: панель настроек сама задаёт раскладку
                в два столбца (меню слева, содержимое справа), и обёртка со
                своим padding сдвинула бы границу между ними. */}
            <div className="ts-sheet__body ts-sheet__body--flush">
              {settings?.(state.client ?? null)}
            </div>
          </div>
        </div>
      )}

      {/* Один конфликт — один диалог: доска получает свой, заметка свой, и
          одновременно они не показываются. */}
      <ConflictDialog
        conflict={boardConflictPath === null ? conflicts.current : null}
        diff={conflicts.state.diff}
        isResolving={conflicts.state.isResolving}
        error={conflicts.state.error}
        onShowDiff={() => void conflicts.showDiff()}
        onHideDiff={conflicts.hideDiff}
        onKeepLocal={() => void conflicts.keepLocal()}
        onAcceptExternal={() => void handleAcceptExternal()}
        onDismissError={conflicts.dismissError}
      />

      <BoardConflictDialog
        boardPath={boardConflictPath}
        localContent={boardVersions.localContent}
        externalContent={boardVersions.externalContent}
        isResolving={conflicts.state.isResolving}
        error={
          conflicts.state.error === null
            ? null
            : describeResolveError(conflicts.state.error)
        }
        onKeepLocal={() => void conflicts.keepLocal()}
        onAcceptExternal={() => void handleAcceptExternalBoard()}
        onDismissError={conflicts.dismissError}
      />
    </main>
  );
}

/**
 * Хлебные крошки шапки по шаблону § Header bar: имя vault, путь и расширение
 * различаются цветом, разделители приглушены сильнее самих сегментов.
 *
 * Строка режется здесь, а не в ядре: это оформление одной надписи, а не
 * правило о путях. Ничего не открыто — показывается только имя vault, чтобы
 * шапка не схлопывалась.
 */
function Breadcrumb({
  rootName,
  path,
}: {
  rootName: string;
  path: string | null;
}): React.JSX.Element {
  const segments = path === null ? [] : path.split("/").filter((part) => part !== "");
  const file = segments.pop() ?? null;
  const dot = file === null ? -1 : file.lastIndexOf(".");
  const stem = file === null ? null : dot > 0 ? file.slice(0, dot) : file;
  const extension = file !== null && dot > 0 ? file.slice(dot) : null;

  return (
    <span className="ts-crumb">
      {rootName}
      {segments.map((segment) => (
        <span key={segment}>
          <span className="ts-crumb__sep"> / </span>
          {segment}
        </span>
      ))}
      {stem !== null && (
        <>
          <span className="ts-crumb__sep"> / </span>
          {stem}
          {extension && <span className="ts-crumb__ext">{extension}</span>}
        </>
      )}
    </span>
  );
}

/**
 * Причина отказа создания как её понимает `ErrorBanner`. `NotABoardName` сюда
 * не попадает: имя по умолчанию — имя доски, и получить этот отказ на нём
 * нельзя.
 */
function asCreateFailure(cause: unknown): CreateError {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const { kind } = cause as { kind: unknown };
    if (kind === "NameConflict") return { kind };
    if (kind === "InvalidName") return { kind };
    if (kind === "AccessDenied") return { kind };
  }
  return { kind: "Io" };
}

/**
 * Сколько папок видно в дереве. Считаются все предки заметки, а не только
 * непосредственная папка: `a/b/note.typ` — это две папки, как их и рисует
 * `buildTree`.
 */
function countFolders(notes: Note[]): number {
  const folders = new Set<string>();
  for (const note of notes) {
    const lastSlash = note.path.lastIndexOf("/");
    let prefix = lastSlash === -1 ? "" : note.path.slice(0, lastSlash);
    while (prefix !== "") {
      folders.add(prefix);
      const parent = prefix.lastIndexOf("/");
      prefix = parent === -1 ? "" : prefix.slice(0, parent);
    }
  }
  return folders.size;
}
