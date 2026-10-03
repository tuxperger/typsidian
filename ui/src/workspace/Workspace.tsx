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
import {
  useVault,
  type LastVault,
  type LastVaultLookup,
  type VaultOpener,
} from "../vault-tree/useVault.js";
import type { VaultClient } from "../vault-client.js";
import { useVaultSettings } from "../settings/useVaultSettings.js";
import { useEditorFontSizeControl } from "../settings/useEditorFontSizeControl.js";

import { defaultNewBoardName } from "../vault-tree/VaultTreeActions.js";
import { Sidebar } from "../vault-tree/Sidebar.js";
import type { DiagnosticLocation } from "../wasm-core/typsidian_core.js";
import { ExportCenter } from "../export/ExportCenter.js";
import { ExportToast } from "../export/ExportToast.js";
import { useExport } from "../export/useExport.js";
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
import { PaneDivider } from "./PaneDivider.js";
import { useDismissOnEscape } from "./useDismissOnEscape.js";
import {
  SIDEBAR_MAX_WIDTH,
  SIDEBAR_MIN_WIDTH,
  SPLIT_MAX,
  SPLIT_MIN,
  usePaneLayout,
} from "./usePaneLayout.js";
import { useShortcuts } from "./useShortcuts.js";
import { GraphView } from "../graph/GraphView.js";
import { SearchOverlay } from "../search/SearchOverlay.js";
import { TemplateGallery } from "../templates/TemplateGallery.js";
import { SaveAsTemplateDialog } from "../templates/SaveAsTemplateDialog.js";
import { useTemplateLibrary } from "../templates/useTemplateLibrary.js";
import { plainInputTarget } from "../templates/insertion/plainInputTarget.js";
import type { EditorView } from "@codemirror/view";
import { ContextMenuProvider, copyText, useContextMenu } from "../menu/ContextMenu.js";
import { useI18n, type MessageKey } from "../i18n/index.js";

export interface WorkspaceProps {
  /**
   * Жест «открыть папку как vault» — единственное, что различается между
   * клиентами (001 T115): в браузере это `showDirectoryPicker`, на рабочем
   * столе — нативный диалог ОС. Всё остальное поведение общее.
   */
  openVaultFolder: VaultOpener;
  /**
   * Последний открытый vault (FR-007). Приходит извне по той же причине, что и
   * жест выбора папки: ссылку на папку хранит хранилище устройства, а оно у
   * каждого клиента своё — `settings.json` через команды Tauri на native,
   * `WebStore` поверх OPFS и IndexedDB в браузере.
   *
   * Не передан — экран приветствия просто не предлагает продолжить. Это не
   * отказ: оболочка без хранилища устройства и не обязана ничего помнить.
   */
  lastVault?: LastVaultLookup;
  /**
   * Панель настроек (фича 004). Приходит извне, а не импортируется здесь,
   * потому что её хранилище платформенное: панель везде одна — `SettingsPanel`,
   * — но клиента хранилища устройства ей передаёт оболочка (`WebStore` поверх
   * OPFS в браузере, `settings.json` за командами Tauri на native).
   *
   * Принимает клиента открытого vault: с фичей 006 у панели появился раздел,
   * работающий не с хранилищем устройства, а с файлом внутри vault — политика
   * расположения файлов вставок и помеченные разделы (FR-002, FR-015).
   * Функция, а не готовый узел: клиент появляется только после открытия
   * папки, и панель обязана увидеть именно текущий.
   */
  settings?: (
    client: VaultClient | null,
    /**
     * Переоткрыть окно на другом vault (US2 фичи 004): панель настроек
     * переключает текущий vault, и дерево обязано показать заметки выбранного,
     * а не остаться на прежних.
     *
     * Жест приходит от панели — она одна знает, какой vault выбран, — а
     * исполняет его рабочая область: клиентом vault владеет она.
     */
    reopen: (opener: VaultOpener) => void,
    /**
     * Клиент предпросмотра (фича 007): через него идёт перечень установленных
     * шаблонов и их явная загрузка. `null` — сборка на этом клиенте ещё не
     * готова, и раздел шаблонов показывает только настройки.
     */
    preview: PreviewClient | null,
  ) => React.ReactNode;
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
export function Workspace(props: WorkspaceProps): React.JSX.Element {
  // Контекстное меню — одно на рабочую область: его открывают дерево, редактор,
  // предпросмотр и шапка, и каждое из них лежит внутри.
  return (
    <ContextMenuProvider>
      <WorkspaceView {...props} />
    </ContextMenuProvider>
  );
}

function WorkspaceView({
  openVaultFolder,
  lastVault,
  settings,
  preview = null,
}: WorkspaceProps): React.JSX.Element {
  const contextMenu = useContextMenu();
  const { t, tEn } = useI18n();
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
  // «Параметры элемента» из палитры (фича 008, FR-006): редактор ловит смену
  // счётчика и открывает форму для элемента под кареткой.
  const [paramsRequest, setParamsRequest] = useState(0);
  const [exportOpen, setExportOpen] = useState(false);
  // Место ошибки, к которому надо перейти, когда нужная заметка откроется в
  // редакторе (009 FR-008, FR-016).
  const [reveal, setReveal] = useState<DiagnosticLocation | null>(null);
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
  // Размеры колонок. Значения по умолчанию — из макета; двигает их
  // пользователь границами между колонками.
  const panes = usePaneLayout();
  // Настройки открытого vault. Нужны здесь троим: предпросмотру (выключатель
  // автозагрузки шаблонов и адреса, указанные пользователем, — фича 007),
  // сочетаниям клавиш и размеру шрифта редактора. Все трое живут в настройках
  // vault, а не устройства, и потому доступны всем трём оболочкам (у desktop и
  // Android хранилища устройства нет).
  const vaultSettings = useVaultSettings(clientForConflicts);
  const shortcuts = useShortcuts(clientForConflicts, vaultSettings.overrides);

  // Размер шрифта редактора доходит до CodeMirror переменной на корне
  // документа. Применяется здесь, а не в панели настроек: панель платформенная
  // и к тому же смонтирована только пока открыта, а размер обязан действовать
  // с первого кадра после открытия папки (FR-017).
  //
  // Отсутствие значения означает «перекрытия нет», а не «13 пикселей»: под
  // этим слоем на web лежит слой устройства, и у него своя переменная — см.
  // `applyToDocument`. Поэтому снятие здесь возвращает размер устройства, а не
  // обнуляет его.
  //
  // Через управление, а не через голое применение: кегль меняется жестом поверх
  // текста и ступенями в статус-баре, и шаг обязан быть видно сразу, а в
  // настройки папки он уезжает после паузы — файл на диск на каждый щелчок
  // колеса писать нельзя.
  const editorFontSize = useEditorFontSizeControl(
    vaultSettings.overrides?.editor_font_size ?? null,
    vaultSettings.setEditorFontSize,
  );

  // Панель настроек — такое же наложение, как палитра и галерея, и закрывается
  // той же клавишей. Сама панель об этом не знает: она платформенная и приходит
  // пропом, а Escape — правило рабочей области.
  const closeSettings = useCallback(() => setShowSettings(false), []);

  // Панель настроек пишет файл vault своим экземпляром `useVaultSettings`, а
  // предпросмотр и сочетания читают этот — общего состояния у них нет. Без
  // перечитывания выключенная в панели автозагрузка шаблонов не доходила бы до
  // сборки, пока папку не переоткроют (007 FR-011: «немедленно»).
  const reloadVaultSettings = vaultSettings.reload;
  const settingsWereShown = useRef(false);
  useEffect(() => {
    if (showSettings) {
      settingsWereShown.current = true;
    } else if (settingsWereShown.current) {
      settingsWereShown.current = false;
      void reloadVaultSettings();
    }
  }, [showSettings, reloadVaultSettings]);
  useDismissOnEscape(showSettings ? closeSettings : null);

  const { state, openVault, openVaultWith, refresh } = useVault(openVaultFolder, {
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

  // Последний открытый vault (FR-007). Спрашивается один раз: после открытия
  // экран приветствия не показывается, а ответ к тому времени уже неверен —
  // последним стал бы открытый сейчас.
  //
  // Отказ хранилища здесь молчит намеренно: предложить продолжить не удалось —
  // остаётся обычный выбор папки, и сообщать об этом пользователю нечего.
  const [offered, setOffered] = useState<LastVault | null>(null);
  // Молчаливое открытие пробуется ровно один раз за жизнь окна. Иначе отказ
  // (папка исчезла между вопросом и открытием) превратился бы в цикл попыток,
  // а отмена пользователя — в невозможность остаться на экране приветствия.
  const autoOpened = useRef(false);
  useEffect(() => {
    if (!lastVault) return;
    let cancelled = false;
    void (async () => {
      let found: LastVault | null = null;
      try {
        found = await lastVault();
      } catch {
        found = null;
      }
      if (cancelled) return;
      setOffered(found);
      // «Мгновенно» — это здесь: запомненный vault открывается сам, без
      // единого нажатия. Кнопка остаётся для случая, когда молчаливо открыть
      // нельзя (в браузере отозвано разрешение на папку) — см. `readyToOpen`.
      if (found?.readyToOpen && !autoOpened.current) {
        autoOpened.current = true;
        await openVaultWith(found.open);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [lastVault, openVaultWith]);

  /**
   * Переоткрыть окно на другом vault (US2 фичи 004, AS3).
   *
   * Открытая заметка и её текст сбрасываются здесь, а не в `useVault`: это
   * состояние рабочей области, и принадлежит оно прежнему vault. Оставить его
   * значило бы показать редактор с заметкой, которой в открытой папке нет.
   */
  const reopenVault = useCallback(
    async (opener: VaultOpener) => {
      setOpenNote(null);
      setEditorText("");
      // Панель настроек закрывается: переключение сделано ради дерева заметок,
      // а оно под ней.
      setShowSettings(false);
      await openVaultWith(opener);
    },
    [openVaultWith],
  );

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

  // Экспорт в PDF (фича 009). Компилятор — тот же, что у предпросмотра: иначе
  // заметка собиралась бы в одной колонке и не собиралась в файле.
  const previewClient = state.preview ?? preview;
  const openNotePath = openNote !== null && !isBoardOpen ? openNote.path.toString() : null;
  const exporter = useExport(previewClient, vaultSettings.overrides, {
    path: openNotePath,
    text: editorText,
  });
  const exportNote = exporter.exportNote;
  const exportCurrent = useCallback(() => {
    // FR-009: экспортировать можно только открытую заметку Typst.
    if (openNotePath !== null) void exportNote(openNotePath, editorText);
  }, [openNotePath, editorText, exportNote]);
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

  // Переход к месту ошибки экспорта (009 FR-008, FR-016): открыть файл, в
  // котором ошибка, и поставить курсор на её строку и колонку. Сам переход
  // случается, когда файл уже в редакторе, — см. эффект ниже.
  const revealDiagnostic = useCallback(
    (place: DiagnosticLocation) => {
      const note = state.notes.find((entry) => entry.path.toString() === place.path);
      if (note === undefined) return;
      setExportOpen(false);
      setView("workspace");
      if (openNote?.path.toString() !== place.path) openEntry(note);
      setReveal(place);
    },
    [state.notes, openNote, openEntry],
  );

  useEffect(() => {
    if (reveal === null || openNote?.path.toString() !== reveal.path) return;
    const view = getEditorView?.() ?? null;
    // Текст заметки ещё не пришёл в редактор — дождаться следующего
    // обновления `editorText`.
    if (view === null || view.state.doc.lines < reveal.line) return;
    const line = view.state.doc.line(reveal.line);
    const position = Math.min(line.to, line.from + Math.max(0, reveal.column - 1));
    view.dispatch({ selection: { anchor: position }, scrollIntoView: true });
    view.focus();
    setReveal(null);
  }, [reveal, openNote, getEditorView, editorText]);

  /**
   * Версия, принятая при разрешении конфликта, — вниз, в редактор (FR-022).
   *
   * Ревизия, а не просто текст: принять внешнюю можно дважды подряд, и вторая
   * такая же строка обязана примениться так же, как первая.
   */
  const [acceptedExternal, setAcceptedExternal] = useState<{
    path: string;
    text: string;
    revision: number;
  } | null>(null);

  const handleAcceptExternal = useCallback(async () => {
    // Путь спрашивается до разрешения: после него конфликт снят и текущего
    // конфликта уже нет.
    const path = conflicts.current?.notePath ?? null;
    const external = await conflicts.acceptExternal();
    // `null` — внешней версии нет: заметку удалили снаружи (FR-025).
    if (external === null) {
      setOpenNote(null);
      return;
    }
    if (path !== null) {
      setAcceptedExternal((previous) => ({
        path,
        text: external,
        revision: (previous?.revision ?? 0) + 1,
      }));
    }
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
      const board = await state.client.createBoard(defaultNewBoardName());
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

  const toggleView = useCallback(() => {
    // Уход с рабочей области обязан дописать доску: дебаунс автосохранения
    // иначе выстрелит в уже размонтированное полотно.
    if (isBoardOpen) void board.flush();
    setView((current) => (current === "workspace" ? "graph" : "workspace"));
  }, [isBoardOpen, board]);

  // Глобальные сочетания. Слушатель здесь, а не в самих наложениях: открыть
  // палитру нужно и тогда, когда её в дереве компонентов ещё нет.
  //
  // Какие именно клавиши — решают настройки уровня vault через ядро
  // (`settings::keybindings`), а не этот файл: раньше они были зашиты прямо
  // здесь, и переназначить их было нечем.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (shortcuts.matches("search", event)) {
        event.preventDefault();
        setSearchOpen((open) => !open);
      } else if (shortcuts.matches("templates", event)) {
        event.preventDefault();
        setTemplatesOpen((open) => !open);
      } else if (shortcuts.matches("command-palette", event)) {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      } else if (shortcuts.matches("toggle-graph", event)) {
        event.preventDefault();
        toggleView();
      } else if (shortcuts.matches("rebuild-preview", event)) {
        event.preventDefault();
        setRebuildToken((token) => token + 1);
      } else if (shortcuts.matches("export-pdf", event)) {
        // `Mod+P` перехватывается и без открытой заметки: иначе браузер
        // показал бы диалог печати страницы приложения, а не заметки.
        event.preventDefault();
        exportCurrent();
      } else if (shortcuts.matches("export-center", event)) {
        event.preventDefault();
        setExportOpen((open) => !open);
      }
      // Escape здесь больше не разбирается: закрытием наложений занят
      // `useDismissOnEscape`, и закрывает он **верхнее** из открытых. Отдельная
      // ветка тут закрывала бы палитру заодно с диалогом поверх неё.
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [shortcuts, toggleView, exportCurrent]);

  /**
   * Команды палитры. Только те, что действительно что-то делают: список из
   * макета содержит экспорт BibTeX и переключение vim-режима, которых в
   * приложении нет, а мёртвая строка в палитре хуже её отсутствия.
   */
  // Ссылка на заметку из контекстного меню дерева — в позицию курсора открытой
  // заметки. Путь от корня vault (`[[/путь]]`): так ссылка не зависит от папки,
  // в которой лежит открытая заметка (`graph::resolve_link_target`).
  const insertLink = useMemo(() => {
    if (getEditorView === null || openNote === null || isBoard(openNote)) return undefined;
    return (target: Note) => {
      const view = getEditorView();
      if (view === null) return;
      view.dispatch(view.state.replaceSelection(`[[/${target.path.toString()}]]`), {
        userEvent: "input",
        scrollIntoView: true,
      });
      view.focus();
    };
  }, [getEditorView, openNote]);

  const commands = useMemo<PaletteCommand[]>(() => {
    // Название на языке интерфейса и по-английски: палитра ищет по обоим
    // (фича 010, FR-024).
    const titled = (key: MessageKey): { title: string; titleEn: string } => ({
      title: t(key),
      titleEn: tEn(key),
    });
    const items: PaletteCommand[] = [
      {
        id: "graph",
        ...titled(view === "graph" ? "workspace.backToWorkspace" : "workspace.openGraph"),
        icon: "network",
        // Подпись берётся у ядра, а не пишется здесь: после переназначения
        // литерал в палитре начал бы врать про клавиши.
        meta: shortcuts.displayOf("toggle-graph"),
        run: toggleView,
      },
      {
        id: "search",
        ...titled("workspace.search"),
        icon: "search",
        meta: shortcuts.displayOf("search"),
        run: () => setSearchOpen(true),
      },
      {
        id: "templates",
        ...titled("workspace.templates"),
        icon: "file",
        meta: shortcuts.displayOf("templates"),
        run: () => setTemplatesOpen(true),
      },
      {
        id: "new-board",
        ...titled("tree.menu.createBoard"),
        icon: "image",
        run: () => void createBoard(),
      },
      {
        id: "rebuild",
        ...titled("workspace.rebuildPreview"),
        icon: "refresh",
        meta: shortcuts.displayOf("rebuild-preview"),
        run: () => setRebuildToken((token) => token + 1),
      },
    ];
    // Только при открытой текстовой заметке: в доске и в графе править
    // параметры элемента Typst негде, а мёртвая строка хуже её отсутствия.
    if (view === "workspace" && openNote !== null && !isBoard(openNote)) {
      items.push({
        id: "element-params",
        ...titled("workspace.elementParams"),
        icon: "terminal",
        meta: shortcuts.displayOf("element-params"),
        run: () => setParamsRequest((request) => request + 1),
      });
    }
    // Фича 009. «Экспорт в PDF» — только при открытой заметке Typst (FR-009);
    // Центр экспорта доступен всегда, пока есть компилятор.
    if (previewClient !== null) {
      if (view === "workspace" && openNotePath !== null) {
        items.push({
          id: "export-pdf",
          ...titled("workspace.exportPdf"),
          icon: "download",
          meta: shortcuts.displayOf("export-pdf"),
          run: exportCurrent,
        });
      }
      items.push({
        id: "export-center",
        ...titled("workspace.exportCenter"),
        icon: "download",
        meta: shortcuts.displayOf("export-center"),
        run: () => setExportOpen(true),
      });
    }
    if (settings !== undefined) {
      items.push({
        id: "settings",
        ...titled("workspace.openSettings"),
        icon: "terminal",
        run: () => setShowSettings(true),
      });
    }
    return items;
  }, [
    view,
    toggleView,
    createBoard,
    settings,
    shortcuts,
    openNote,
    previewClient,
    openNotePath,
    exportCurrent,
    t,
    tEn,
  ]);

  if (!state.client) {
    return (
      <main className="ts-welcome">
        <h1 className="ts-welcome__mark">
          <span className="ts-wordmark__slash">/</span>typsidian
        </h1>
        <p>{t("workspace.welcome")}</p>
        <OpenVault
          isLoading={state.isLoading}
          error={state.error}
          onOpen={openVault}
          last={offered}
          {...(offered ? { onOpenLast: () => void openVaultWith(offered.open) } : {})}
        />
      </main>
    );
  }

  return (
    <main className="ts-app">
      {/* Определения иконок монтируются один раз на всё приложение; всё
          остальное ссылается на них через `<use>`. */}
      <IconSprite />

      <header
        className="ts-header"
        onContextMenu={(event) => {
          const path = openNote?.path.toString() ?? null;
          contextMenu.open(event, [
            ...(path === null
              ? []
              : [
                  {
                    label: t("workspace.copyNotePath"),
                    run: async () => {
                      const failure = await copyText(path);
                      contextMenu.notify(failure ?? t("tree.menu.copied", { path }));
                    },
                  },
                  { kind: "separator" as const },
                ]),
            {
              label: panes.sidebarHidden ? t("workspace.showTree") : t("workspace.hideTree"),
              run: panes.toggleSidebar,
            },
            {
              label: view === "graph" ? t("workspace.backToWorkspace") : t("workspace.openGraph"),
              shortcut: shortcuts.displayOf("toggle-graph"),
              run: toggleView,
            },
            ...(view === "workspace"
              ? [
                  {
                    label: t("workspace.rebuildPreview"),
                    shortcut: shortcuts.displayOf("rebuild-preview"),
                    disabled: compile.isBuilding,
                    run: () => setRebuildToken((token) => token + 1),
                  },
                ]
              : []),
            { kind: "separator" },
            {
              label: t("palette.label"),
              shortcut: shortcuts.displayOf("command-palette"),
              run: () => setPaletteOpen(true),
            },
            {
              label: t("workspace.search"),
              shortcut: shortcuts.displayOf("search"),
              run: () => setSearchOpen(true),
            },
            ...(settings !== undefined
              ? [{ label: t("workspace.settings"), run: () => setShowSettings(true) }]
              : []),
          ]);
        }}
      >
        <div className="ts-header__group">
          {/* Дерево файлов убирается целиком — тексту и предпросмотру остаётся
              всё окно. Кнопка в шапке, а не в самой панели: скрытой панели на
              экране нет, и вернуть её было бы нечем. */}
          <button
            type="button"
            className="ts-btn ts-btn--icon"
            aria-pressed={!panes.sidebarHidden}
            title={panes.sidebarHidden ? t("workspace.showTree") : t("workspace.hideTree")}
            aria-label={
              panes.sidebarHidden ? t("workspace.showTree") : t("workspace.hideTree")
            }
            onClick={panes.toggleSidebar}
          >
            <Icon name="layout" size={14} />
          </button>
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
                  <div className="ts-hazard" role="progressbar" aria-label={t("workspace.compile.building")}>
                    <div className="ts-hazard__fill" />
                  </div>
                  <span className="ts-compile__label">{t("workspace.compile.compiling")}</span>
                </>
              ) : (
                <>
                  <span className="ts-compile__dot" aria-hidden="true" />
                  <span className="ts-compile__label">
                    {compile.lastMs === null
                      ? t("workspace.compile.ready")
                      : t("workspace.compile.readyIn", { ms: compile.lastMs })}
                  </span>
                </>
              )}
              <button
                type="button"
                className="ts-btn ts-btn--icon"
                title={t("workspace.rebuild")}
                aria-label={t("workspace.rebuildPreview")}
                disabled={compile.isBuilding}
                onClick={() => setRebuildToken((token) => token + 1)}
              >
                <Icon name="refresh" />
              </button>
            </div>
          )}

          {/* Ход пакета экспорта виден и при закрытом Центре (009 FR-025). */}
          {exporter.batchRunning && exporter.batch !== null && (
            <button
              type="button"
              className="ts-export-pill"
              title={t("workspace.openExportCenter")}
              onClick={() => setExportOpen(true)}
            >
              <span className="ts-hazard" aria-hidden="true">
                <span className="ts-hazard__fill" />
              </span>
              {t("workspace.exportProgress", {
                processed: exporter.batch.processed,
                total: exporter.batch.total,
              })}
            </button>
          )}

          <span className="ts-divider" />

          <button
            type="button"
            className="ts-btn"
            aria-pressed={view === "graph"}
            title={t("tree.graph.title")}
            onClick={toggleView}
          >
            {view === "workspace" ? (
              <Icon name="network" size={14} />
            ) : (
              <Icon name="chevron" size={14} className="ts-icon--flip" />
            )}
            {view === "workspace" ? t("workspace.graph") : t("graph.back")}
          </button>

          {/* Кнопка показывает действующее сочетание, а не ⌘K из макета:
              после переназначения подпись обязана следовать за клавишами.
              Значок ⌘ остаётся, только пока сочетание его содержит. */}
          <button
            type="button"
            className="ts-btn ts-btn--cmd"
            title={t("palette.label")}
            aria-label={t("workspace.openPalette")}
            onClick={() => setPaletteOpen(true)}
          >
            {shortcuts.displayOf("command-palette") || (
              <>
                <Icon name="cmd" size={12} />
                K
              </>
            )}
          </button>

          {settings !== undefined && (
            <button
              type="button"
              className="ts-btn"
              aria-pressed={showSettings}
              onClick={() => setShowSettings((shown) => !shown)}
            >
              {showSettings ? t("workspace.closeSettings") : t("workspace.settings")}
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
        {exporter.last !== null && (
          <ExportToast
            result={exporter.last}
            onDismiss={exporter.dismissLast}
            onReveal={revealDiagnostic}
            onOpenCenter={() => setExportOpen(true)}
          />
        )}
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
      <div className="ts-body" ref={panes.bodyRef} style={panes.style}>
        {/* Скрытое дерево не рендерится вовсе, а не сжимается до нуля: панель
            нулевой ширины оставляла бы в раскладке свою рамку и по-прежнему
            читала бы vault ради счётчика в подвале. */}
        {!panes.sidebarHidden && (
          <>
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
              shortcuts={shortcuts}
              onInsertLink={insertLink}
            />

            {/* Граница панели и остального окна. Стоит и при открытой доске:
                доска занимает место пары «редактор + предпросмотр», а панель —
                нет. */}
            <PaneDivider
              label={t("workspace.sidebarWidth")}
              now={panes.sidebarWidth}
              min={SIDEBAR_MIN_WIDTH}
              max={SIDEBAR_MAX_WIDTH}
              onDrag={panes.dragSidebar}
              onStep={panes.stepSidebar}
              onReset={panes.resetSidebar}
            />
          </>
        )}

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
              acceptedExternal={acceptedExternal}
              onOpenBoard={openBoard}
              onCreateBoard={() => void createBoard()}
              onContentChange={setEditorText}
              onEditorReady={editorReady}
              onSaveAsTemplate={setSavingTemplate}
              fontSize={editorFontSize}
              shortcuts={shortcuts}
              paramsRequest={paramsRequest}
            />

            {/* Доля считается в процентах, а не в пикселях: ширина колонки
                зависит от размера окна, а доля — нет, и озвучивать пиксели,
                меняющиеся сами по себе, значило бы врать скринридеру. */}
            <PaneDivider
              label={t("workspace.editorPreviewSplit")}
              now={panes.editorShare * 100}
              min={SPLIT_MIN * 100}
              max={SPLIT_MAX * 100}
              onDrag={panes.dragSplit}
              onStep={panes.stepSplit}
              onReset={panes.resetSplit}
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
              vaultOverrides={vaultSettings.overrides}
              // Фича 009: кнопка «PDF» в тулбаре — только у открытой заметки.
              onExport={openNotePath === null ? undefined : exportCurrent}
              exporting={exporter.busy}
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

      {exportOpen && previewClient !== null && (
        <ExportCenter
          client={previewClient}
          overrides={vaultSettings.overrides}
          exporter={exporter}
          preselect={openNotePath}
          onClose={() => setExportOpen(false)}
          onOpenSettings={() => {
            setExportOpen(false);
            setShowSettings(true);
          }}
          onReveal={revealDiagnostic}
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
          suggestedName={openNote?.name ?? t("workspace.templateName")}
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
              <span className="ts-eyebrow">{t("workspace.settings")}</span>
              <span className="ts-spacer" />
              <button
                type="button"
                className="ts-btn"
                onClick={() => setShowSettings(false)}
              >
                {t("common.close")}
              </button>
            </div>
            {/* Без внутренних отступов: панель настроек сама задаёт раскладку
                в два столбца (меню слева, содержимое справа), и обёртка со
                своим padding сдвинула бы границу между ними. */}
            <div className="ts-sheet__body ts-sheet__body--flush">
              {settings?.(
                state.client ?? null,
                (opener) => void reopenVault(opener),
                state.preview ?? preview ?? null,
              )}
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
            : describeResolveError(t, conflicts.state.error)
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
