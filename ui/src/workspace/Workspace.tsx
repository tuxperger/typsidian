"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ConflictDialog } from "../editor/ConflictDialog.js";
import {
  describeResolveError,
  useConflictResolution,
} from "../editor/useConflictResolution.js";
import { useWindowCloseGuard } from "../editor/useWindowCloseGuard.js";
import { NoteDocumentRegistry } from "../editor/noteDocuments.js";
import { tabEditorStates } from "../editor/tabEditorStates.js";
import { useDocumentSnapshot, useNoteDocument } from "../editor/useNoteDocument.js";
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
import type {
  Condition,
  OpenText,
  OutlineRow as OutlineRowView,
  TextPlace,
  WorkspaceView,
} from "../wasm-core/typsidian_core.js";
import { ExportCenter } from "../export/ExportCenter.js";
import { ExportToast } from "../export/ExportToast.js";
import { useAttachments } from "../attachments/useAttachments.js";
import { AttachmentNotices } from "../attachments/AttachmentNotices.js";
import { LargeImageDialog } from "../attachments/LargeImageDialog.js";
import { InsertImageSheet, type ImageSources } from "../attachments/InsertImageSheet.js";
import { pickFiles, type PickSource } from "../attachments/pickFiles.js";
import { IgnoreEditor } from "../sync/IgnoreEditor.js";
import { useExport } from "../export/useExport.js";
import { ImportDialog } from "../import/ImportDialog.js";
import { ImportProgress } from "../import/ImportProgress.js";
import type { ImportSourcePicker } from "../import/import-source.js";
import { useImport } from "../import/useImport.js";
import type {
  CreateError,
  Note,
  Template,
  VaultEvent,
} from "../wasm-core/typsidian_core.js";
import { BoardConflictDialog } from "../boards/BoardConflictDialog.js";
import { BoardControllers } from "../boards/boardControllers.js";
import { useBoardConflict } from "../boards/useBoardConflict.js";
import { isBoard } from "../vault-tree/fileKind.js";
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
import { hintWith, useShortcuts } from "./useShortcuts.js";
import { EditorGroups } from "./tabs/EditorGroups.js";
import { OpenNotesSheet } from "./tabs/OpenNotesSheet.js";
import type { WorkspaceTabsStore } from "./tabs/tabs-store.js";
import { useWindowWidth } from "./tabs/useWindowWidth.js";
import { useWorkspaceTabs } from "./tabs/useWorkspaceTabs.js";
import { GraphView, type GraphScope } from "../graph/GraphView.js";
import { LocalGraphMode } from "../graph/LocalGraphMode.js";
import { useGraphSettings } from "../graph/useGraphSettings.js";
import { SearchOverlay } from "../search/SearchOverlay.js";
import { PropertiesMode } from "../note-panel/properties/PropertiesMode.js";
import { useNoteProperties } from "../note-panel/properties/useNoteProperties.js";
import { usePropertiesWorkspace } from "../note-panel/properties/usePropertiesWorkspace.js";
import { TemplateGallery } from "../templates/TemplateGallery.js";
import { SaveAsTemplateDialog } from "../templates/SaveAsTemplateDialog.js";
import { useTemplateLibrary } from "../templates/useTemplateLibrary.js";
import { plainInputTarget } from "../templates/insertion/plainInputTarget.js";
import type { EditorView } from "@codemirror/view";
import { ContextMenuProvider, copyText, useContextMenu } from "../menu/ContextMenu.js";
import { useI18n, type MessageKey } from "../i18n/index.js";
import { FileHistory } from "../sync/FileHistory.js";
import { RemoteDialog } from "../sync/RemoteDialog.js";
import { SyncIndicator } from "../sync/SyncIndicator.js";
import { KeeperConfirmDialog, SyncPanel } from "../sync/SyncPanel.js";
import {
  asSyncError,
  type CloneRequest,
  type RemoteCapabilities,
  type VaultCloner,
} from "../sync/sync-client.js";
import { useSync, type SyncControl } from "../sync/useSync.js";
import type { IgnoreFile, SyncError } from "../wasm-core/typsidian_core.js";
import type { DeviceSettingsClient } from "../settings/device-settings-client.js";
import { NotePanel } from "../note-panel/NotePanel.js";
import type { NotePanelModeSpec } from "../note-panel/modes.js";
import { useNarrowViewport } from "../note-panel/useNarrowViewport.js";
import { usePhoneViewport } from "./usePhoneViewport.js";
import { useNotePanelState } from "../note-panel/useNotePanelState.js";
import { NotePanelDrawer, useBodyBelow } from "../note-panel/NotePanelDrawer.js";
import { OutlineMode, type OutlineTarget } from "../note-panel/OutlineMode.js";
import { useOutline } from "../note-panel/useOutline.js";
import type { HeadingRequest } from "../preview/PreviewPane.js";
import { LinksMode, type RevealPlace } from "../note-panel/links/LinksMode.js";
import { plainEditors } from "../note-panel/links/plainEditors.js";
import { useNoteLinks, type LinksChanges } from "../note-panel/links/useNoteLinks.js";
import type { PreviewState } from "../preview/useCompiledDocument.js";
import type { ShowHost } from "../slides/show-channel.js";
import { ShowMenu } from "../slides/ShowMenu.js";
import { useShowSession, type PreviewSnapshot } from "../slides/useShowSession.js";
import type { CaptureSource } from "../capture/capture-source.js";
import { CaptureSheet } from "../capture/CaptureSheet.js";
import { DailyNav } from "../capture/DailyNav.js";
import { useCapture } from "../capture/useCapture.js";
import { useDailyNote } from "../capture/useDailyNote.js";
import { useSources } from "../sources/useSources.js";
import { SourcesPanel } from "../sources/SourcesPanel.js";
import { createErrorText } from "../sources/describe.js";
import type { NoteCompletion, NoteSources } from "../editor/NoteEditor.js";
import { useLinkedIndex } from "../completion/useLinkedIndex.js";
import { useCompletionAuto } from "../completion/useCompletionAuto.js";
import { authorsLine } from "../sources/describe.js";
import type { DefinitionOutcome, Source } from "../wasm-core/typsidian_core.js";

/** Тексты заметок для связей, пока vault не открыт. */
const NO_OPEN_TEXTS: readonly OpenText[] = [];

/** Колонка, которую показывает телефон (010 T057). */
type PhonePane = "tree" | "editor" | "preview";

const PHONE_PANE_LABEL = {
  tree: "workspace.phone.tree",
  editor: "workspace.phone.editor",
  preview: "workspace.phone.preview",
} as const satisfies Record<PhonePane, MessageKey>;

/** Указанных пользователем файлов источников нет (фича 017). */
const NO_SOURCE_FILES: readonly string[] = [];

/** Конец заметки: редактор прижимает строку и колонку к последним (фича 022). */
const END_OF_NOTE: TextPlace = { line: 1_000_000_000, column: 1_000_000_000 };

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
    /**
     * Синхронизация открытого vault (фича 011) — раздел её настроек живёт в
     * панели настроек; и жест «создать vault из удалённого репозитория» для
     * раздела vault. `null` — недоступно: vault не открыт или оболочка жеста
     * не даёт.
     */
    extras: { sync: SyncControl | null; onCloneVault: (() => void) | null },
  ) => React.ReactNode;
  /**
   * Компилятор предпросмотра (фича 005). Приходит извне по той же причине, что
   * и панель настроек: на web это wasm в том же процессе, на рабочем столе —
   * команда Tauri. Пока клиент не передан, колонка предпросмотра честно
   * сообщает, что сборка недоступна, вместо того чтобы делать вид, что она
   * есть.
   */
  preview?: PreviewClient | null;
  /**
   * Жест «создать vault из удалённого репозитория» (фича 011, FR-013).
   * Платформенный по той же причине, что и `openVaultFolder`: пустую папку
   * выбирает оболочка. Не передан — действие не предлагается.
   */
  cloneVault?: VaultCloner;
  /**
   * Хранилище настроек устройства (фича 013): в нём лежит состояние колонки
   * заметки — показана ли, какой режим, раскрыты ли упоминания. Не передан —
   * состояние живёт в памяти окна.
   */
  device?: DeviceSettingsClient;
  /**
   * Хранилище набора вкладок на устройстве (фича 014, FR-021…FR-023). Приходит
   * извне по той же причине, что и `device`: способ достучаться до настроек
   * устройства у каждой оболочки свой. Не передан — вкладки работают, но не
   * переживают перезапуск.
   */
  tabsStore?: WorkspaceTabsStore;
  /**
   * Показ слайдов (фича 019): хозяин канала окон показа. Приходит от оболочки по
   * той же причине, что и `openVaultFolder`: окно показа — платформенное
   * (`window.open` в web, `WebviewWindow` на рабочем столе, слой в памяти на
   * Android). Не передан — кнопки «Показ» нет.
   */
  showHost?: ShowHost;
  /**
   * Источники изображений оболочки (фича 016, research.md §2): камера и
   * галерея есть только на телефоне. Умолчание — ни одного: «Вставить
   * изображение…» тогда сразу открывает выбор файлов.
   */
  imageSources?: ImageSources;
  /**
   * Выбор исходной папки импорта из Obsidian (фича 021, FR-001). Платформенный
   * жест по той же причине, что и `openVaultFolder`: в браузере это
   * `showDirectoryPicker` в режиме чтения, на рабочем столе и Android — диалог
   * или обзор папок. Не передан — команды импорта нет; `available: false` —
   * команда есть, а диалог называет причину.
   */
  pickImportSource?: ImportSourcePicker;
  /**
   * Приём «Поделиться» и ярлыков приложения (фича 022). Платформенный по той же
   * причине, что и `showHost`: намерения Android недоступны ни из Rust, ни из JS.
   * Не передан — приёма нет, а команды «Во входящие» и «Сегодня» работают везде.
   */
  captureSource?: CaptureSource;
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
const NO_IMAGE_SOURCES: ImageSources = { camera: false, gallery: false };


function WorkspaceView({
  openVaultFolder,
  lastVault,
  settings,
  preview = null,
  cloneVault,
  device,
  tabsStore,
  showHost,
  imageSources = NO_IMAGE_SOURCES,
  pickImportSource,
  captureSource,
}: WorkspaceProps): React.JSX.Element {
  const contextMenu = useContextMenu();
  const { t, tEn, lang, formatNumber } = useI18n();
  const [operationError, setOperationError] = useState<VaultOperationError | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  // Экран 2 макета — отдельный вид, а не панель: граф занимает всё окно.
  const [view, setView] = useState<"workspace" | "graph">("workspace");
  const [paletteOpen, setPaletteOpen] = useState(false);
  // Расширенный поиск (макет «Утилиты» § Поиск): ⌘⇧F и команда палитры.
  const [searchOpen, setSearchOpen] = useState(false);
  // Условия по свойствам, с которыми открывается поиск (клик по тегу, фича 020).
  const [searchConditions, setSearchConditions] = useState<Condition[]>([]);
  // Запрос формы блока выборки из поиска (Mod+Enter, макет B3 -> B4).
  const [selectRequest, setSelectRequest] = useState<{ id: number; conditions: Condition[] } | null>(null);
  // select.typ правили вручную и оставили как есть: предупреждение в колонке свойств.
  const [libraryCustomized, setLibraryCustomized] = useState(false);
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
  // Доски во вкладках: как их дописать при закрытии вкладки и чья запись не удалась.
  const boardControllers = useMemo(() => new BoardControllers(), []);
  // Открыта ли шторка «Открытые заметки» (телефон, фича 014).
  const [openNotesSheet, setOpenNotesSheet] = useState(false);
  // Вкладки читают состояние через ссылки: событие vault и переход документа
  // «чисто ↔ несохранённое» приходят раньше, чем хук вкладок создан.
  const tabsEventRef = useRef<((event: VaultEvent) => void) | null>(null);
  // Библиотека источников (фича 017) появляется позже колбэка событий.
  const sourcesEventRef = useRef<((event: VaultEvent) => void) | null>(null);
  const tabsRefreshRef = useRef<(() => void) | null>(null);
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

  // Счётчик обновления колонки заметки (фича 013, research.md §14): растёт на
  // события vault, удачное автосохранение и итог «Связать». Какие заметки
  // затронуты, копится отдельно и забирается хуком при каждом увеличении:
  // события одного кадра React схлопывает в одно состояние.
  const [linksRevision, setLinksRevision] = useState(0);
  const linksChanges = useRef<LinksChanges>({ all: false, paths: new Set() });
  const bumpLinks = useCallback((paths: readonly string[] | null) => {
    if (paths === null) linksChanges.current.all = true;
    else for (const path of paths) linksChanges.current.paths.add(path);
    setLinksRevision((count) => count + 1);
  }, []);
  const takeLinksChanges = useCallback((): LinksChanges => {
    const taken = linksChanges.current;
    linksChanges.current = { all: false, paths: new Set() };
    return taken;
  }, []);

  // Клиент появляется позже этого колбэка, поэтому берётся из ref: сам колбэк
  // обязан быть стабильным — он уезжает в подписку на события.
  const clientRef = useRef<VaultClient | null>(null);
  // Синхронизация (фича 011) появляется позже этого колбэка — по той же
  // причине берётся из ref.
  const syncRef = useRef<SyncControl | null>(null);
  // Связанные файлы показанной сборки (005 FR-020): изменение любого из них
  // пересобирает предпросмотр. Что считать связанным, сказало ядро — здесь
  // только сверка пути события со списком.
  // Те же пути — списком для меток связанных файлов (018); тот же список
  // после очередной сборки состояния не меняет.
  const [previewDependencies, setPreviewDependencies] = useState<readonly string[]>([]);
  // Заметки появились или исчезли — отметки «заметки нет» пересчитываются (018).
  const [notesRevision, setNotesRevision] = useState(0);
  const previewDependenciesRef = useRef<ReadonlySet<string>>(new Set());
  const notePreviewDependencies = useCallback((paths: string[]) => {
    previewDependenciesRef.current = new Set(paths);
    setPreviewDependencies((previous) =>
      previous.length === paths.length && previous.every((path, index) => path === paths[index])
        ? previous
        : paths,
    );
  }, []);
  const noticeEvent = useCallback((event: VaultEvent) => {
    if (touchesAny(event, previewDependenciesRef.current)) {
      setRebuildToken((token) => token + 1);
    }
    // Любое изменение файлов — повод перечитать сводку синхронизации и начать
    // отсчёт паузы автоснимка заново; что из этого следует, решает ядро.
    syncRef.current?.noticeVaultChange();
    // Вкладки: переименование, удаление и внешнее изменение заметки.
    tabsEventRef.current?.(event);
    // Источники: файлы библиотеки и цитаты заметок (фича 017).
    sourcesEventRef.current?.(event);
    // Колонка заметки: что именно изменилось, определяет вид события. Переименование
    // и конец массового изменения путей не называют — упоминания ищутся заново;
    // `LinksInvalidated` меняет только ссылки.
    if (
      event.type === "NoteCreated" ||
      event.type === "NoteDeleted" ||
      event.type === "NoteRenamed" ||
      event.type === "BulkChangeSettled"
    ) {
      setNotesRevision((count) => count + 1);
    }
    switch (event.type) {
      case "NoteCreated":
      case "NoteUpdated":
      case "NoteDeleted":
        bumpLinks([event.path]);
        break;
      case "NoteRenamed":
      case "BulkChangeSettled":
        bumpLinks(null);
        break;
      case "LinksInvalidated":
        bumpLinks([]);
        break;
      default:
        break;
    }
    void (async () => {
      const effect = await clientRef.current?.templateEvent(event);
      // Оба варианта — повод перечитать состав: у `TemplateChanged` меняется
      // отпечаток заготовки, а по нему карточка пересчитывает превью.
      if (effect !== undefined) setLibraryRevision((count) => count + 1);
    })();
  }, [bumpLinks]);

  // Конфликты приходят событиями из ядра; решение принимает пользователь в
  // диалоге, а не эта оболочка (FR-021).
  const [clientForConflicts, setClientForConflicts] = useState<
    Parameters<typeof useConflictResolution>[0]
  >(null);
  // Диалог показывается для конфликта активной вкладки; остальные ждут с
  // признаком на своей вкладке (фича 014, research.md §7).
  const [conflictFocus, setConflictFocus] = useState<string | null>(null);
  const conflicts = useConflictResolution(clientForConflicts, conflictFocus);
  // Размеры колонок. Значения по умолчанию — из макета; двигает их
  // пользователь границами между колонками.
  const panes = usePaneLayout();
  // Колонка заметки (фича 013): состояние лежит в настройках устройства. На узком
  // окне вместо колонки — шторка снизу, на средней ширине `.ts-body` — боковая
  // шторка (фича 015, research.md §11); открыта ли шторка, не сохраняется
  // (013 research.md §12). Обе шторки делят один признак «открыта».
  const notePanel = useNotePanelState(device);
  const narrow = useNarrowViewport();
  const drawer = useBodyBelow(panes.bodyRef) && !narrow;
  const [noteSheetOpen, setNoteSheetOpen] = useState(false);
  const notePanelOpen = narrow || drawer ? noteSheetOpen : notePanel.shown;
  const toggleNotePanel = useCallback(() => {
    if (narrow || drawer) setNoteSheetOpen((open) => !open);
    else notePanel.toggle();
  }, [narrow, drawer, notePanel.toggle]);
  const closeNoteSheet = useCallback(() => setNoteSheetOpen(false), []);
  // «Оглавление» (фича 015, research.md §12): скрыта — показать в режиме
  // «Структура»; показана в другом режиме — переключить; показана в нём —
  // скрыть. Шторку команда открывает и закрывает так же.
  const notePanelMode = notePanel.mode;
  const { setMode: setNotePanelMode, setShown: setNotePanelShown } = notePanel;
  const notePanelShown = notePanel.shown;
  const toggleOutline = useCallback(() => {
    if (narrow || drawer) {
      if (noteSheetOpen && notePanelMode === "outline") {
        setNoteSheetOpen(false);
      } else {
        setNotePanelMode("outline");
        setNoteSheetOpen(true);
      }
    } else if (!notePanelShown) {
      setNotePanelMode("outline");
      setNotePanelShown(true);
    } else if (notePanelMode !== "outline") {
      setNotePanelMode("outline");
    } else {
      setNotePanelShown(false);
    }
  }, [narrow, drawer, noteSheetOpen, notePanelMode, notePanelShown, setNotePanelMode, setNotePanelShown]);
  // «Источники» (фича 017) — тем же порядком, что «Оглавление».
  const toggleSources = useCallback(() => {
    if (narrow || drawer) {
      if (noteSheetOpen && notePanelMode === "sources") {
        setNoteSheetOpen(false);
      } else {
        setNotePanelMode("sources");
        setNoteSheetOpen(true);
      }
    } else if (!notePanelShown) {
      setNotePanelMode("sources");
      setNotePanelShown(true);
    } else if (notePanelMode !== "sources") {
      setNotePanelMode("sources");
    } else {
      setNotePanelShown(false);
    }
  }, [narrow, drawer, noteSheetOpen, notePanelMode, notePanelShown, setNotePanelMode, setNotePanelShown]);
  // Показать скрытое дерево из команды палитры (фича 023). Через ref: функция
  // раскладки новая на каждую отрисовку, а список команд от неё зависеть не
  // должен.
  const sidebarHidden = panes.sidebarHidden;
  const toggleSidebarRef = useRef(panes.toggleSidebar);
  toggleSidebarRef.current = panes.toggleSidebar;
  const showSidebar = useCallback(() => toggleSidebarRef.current(), []);
  // Настройки открытого vault. Нужны здесь троим: предпросмотру (выключатель
  // автозагрузки шаблонов и адреса, указанные пользователем, — фича 007),
  // сочетаниям клавиш и размеру шрифта редактора. Все трое живут в настройках
  // vault, а не устройства, и потому доступны всем трём оболочкам (у desktop и
  // Android хранилища устройства нет).
  const vaultSettings = useVaultSettings(clientForConflicts);
  const shortcuts = useShortcuts(clientForConflicts, vaultSettings.overrides);

  // Настройки графа (фича 025) общие для экрана графа и режима колонки: ключ `graph` в vault.json.
  const graphSettings = useGraphSettings(vaultSettings.overrides, vaultSettings.setGraph);
  const [graphScope, setGraphScope] = useState<GraphScope>("vault");
  const [graphDepth, setGraphDepth] = useState(2);
  const [columnDepth, setColumnDepth] = useState(1);

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
    // Вкладки закрываются по событию удаления через `noticeEvent`.
    onNoteDeleted: fileChanged,
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
   * Вкладки прежнего vault сбрасывает хук вкладок при смене клиента (набор
   * уходит на устройство), а не `useVault`: это состояние рабочей области, и
   * принадлежит оно прежнему vault. Оставить его значило бы показать редактор с
   * заметкой, которой в открытой папке нет.
   */
  const reopenVault = useCallback(
    async (opener: VaultOpener) => {
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

  // ── Вкладки (фича 014) ───────────────────────────────────────────────────
  //
  // Модель вкладок — в ядре; здесь значение состояния, очередь действий и показ.
  // Все потребители заметки (предпросмотр, экспорт, крошки, дерево, граф, поиск,
  // история файла, колонка заметки) берут её из активной вкладки активной группы
  // (`layout.active`), а текст — из её документа (FR-009).
  const registry = useMemo(
    () =>
      state.client === null
        ? null
        : new NoteDocumentRegistry(state.client, {
            // `write` событий наружу не публикует: колонка заметки обновляется по этому сигналу.
            // Индекс цитат (фича 017) — по тому же сигналу.
            onSaved: (path) => {
              bumpLinks([path]);
              sourcesEventRef.current?.({ type: "NoteUpdated", path });
            },
            onDirtyChange: () => tabsRefreshRef.current?.(),
          }),
    [state.client, bumpLinks],
  );
  useEffect(() => () => registry?.dispose(), [registry]);
  const widthPx = useWindowWidth();
  const tabs = useWorkspaceTabs({
    client: state.client,
    registry,
    boards: boardControllers,
    store: tabsStore,
    widthPx,
  });
  tabsEventRef.current = tabs.noticeEvent;
  tabsRefreshRef.current = tabs.refresh;
  const layout = tabs.view;
  const active = layout?.active ?? null;
  const activePath = active?.path ?? null;
  // Вид открытого файла берёт ядро (`ActiveNote.kind`), а не расширение здесь
  // (принцип I): вкладка может быть и доской.
  const isBoardOpen = active?.kind === "Board";
  const { open: openTab, dispatch: dispatchTab } = tabs;
  // Колонка предпросмотра: доска при одной группе её не имеет (research.md §9).
  const showPreview = !(isBoardOpen && (layout?.groups.length ?? 1) < 2);

  // Телефон (010 T057, SC-006): дерево, текст и предпросмотр — по одному, с
  // переключателем внизу. Открытая заметка сама переводит к тексту.
  const phone = usePhoneViewport();
  const [phonePane, setPhonePane] = useState<PhonePane>("tree");
  const lastPhonePath = useRef<string | null>(null);
  useEffect(() => {
    if (activePath !== null && activePath !== lastPhonePath.current) setPhonePane("editor");
    lastPhonePath.current = activePath;
  }, [activePath]);
  const shownPhonePane: PhonePane =
    phonePane === "preview" && !showPreview ? "editor" : phonePane;
  const activeNote = useMemo(
    () => (activePath === null ? null : (state.notes.find((entry) => entry.path === activePath) ?? null)),
    [activePath, state.notes],
  );

  // Текст активной заметки из её документа: предпросмотр собирается из буфера, а
  // не из файла на диске (FR-002). Документ общий для всех видов заметки.
  const openNotePath = active !== null && !isBoardOpen ? active.path : null;
  const activeDocument = useNoteDocument(registry, openNotePath);
  const activeSnapshot = useDocumentSnapshot(activeDocument);
  const editorText =
    activeDocument !== null && activeDocument.path === openNotePath ? activeSnapshot.text : "";

  // Вложения (фича 016): пачка живёт здесь — у неё одна очередь на окно, а
  // принимают файлы редакторы всех групп.
  const attachments = useAttachments(
    state.client,
    vaultSettings.overrides,
    openNotePath,
    getEditorView,
  );
  // Источники (фича 017): библиотека одна на окно, редакторы получают её срез
  // и запросы «вставить цитату».
  const sources = useSources(
    state.client,
    vaultSettings.overrides?.source_files ?? NO_SOURCE_FILES,
    vaultSettings.setSourceFiles,
  );
  sourcesEventRef.current = sources.onEvent;
  const [citeRequest, setCiteRequest] = useState<{ id: number; source: Source | null } | null>(null);
  const declinedBibliography = useRef(new Set<string>());
  const requestCitation = useCallback((source: Source | null) => {
    setCiteRequest((previous) => ({ id: (previous?.id ?? 0) + 1, source }));
  }, []);
  const insertCitationCommand = useCallback(() => {
    if (openNotePath !== null) requestCitation(null);
  }, [openNotePath, requestCitation]);
  const sourcesReadOnly = state.accessState === "readOnly";
  const { createFile: createSourceFile, registerFile: registerSourceFile } = sources;
  const noteSources = useMemo<NoteSources>(
    () => ({
      revision: sources.revision,
      summary: sources.summary,
      readOnly: sourcesReadOnly,
      request: citeRequest,
      createFile: createSourceFile,
      registerFile: registerSourceFile,
      declined: declinedBibliography.current,
    }),
    [sources.revision, sources.summary, sourcesReadOnly, citeRequest, createSourceFile, registerSourceFile],
  );
  const createSourceFileFromPanel = useCallback(async () => {
    const failure = await createSourceFile();
    return failure === null ? t("sources.empty.created") : createErrorText(t, failure);
  }, [createSourceFile, t]);
  const registerSourceFileFromPanel = useCallback(
    (path: string) => void registerSourceFile(path),
    [registerSourceFile],
  );
  const [imageSheetOpen, setImageSheetOpen] = useState(false);
  const [rulesFile, setRulesFile] = useState<IgnoreFile | null>(null);
  const { insertAtCaret: insertImagesAtCaret } = attachments;
  const pickImages = useCallback(
    (source: PickSource) => {
      void pickFiles(source).then((files) =>
        insertImagesAtCaret(files, source === "camera" ? "Camera" : "File"),
      );
    },
    [insertImagesAtCaret],
  );
  // «Вставить изображение…» (FR-004): на телефоне — шторка источников, в
  // остальных оболочках — сразу выбор файлов.
  const insertImage = useCallback(() => {
    if (openNotePath === null) return;
    if (imageSources.camera || imageSources.gallery) setImageSheetOpen(true);
    else pickImages("files");
  }, [openNotePath, imageSources, pickImages]);
  const noteAttachments = useMemo(
    () => ({
      insert: attachments.insert,
      linkDropped: attachments.linkDropped,
      pickAndInsert: insertImage,
    }),
    [attachments.insert, attachments.linkDropped, insertImage],
  );

  // Экспорт в PDF (фича 009). Компилятор — тот же, что у предпросмотра: иначе
  // заметка собиралась бы в одной колонке и не собиралась в файле.
  const previewClient = state.preview ?? preview;
  const exporter = useExport(previewClient, vaultSettings.overrides, {
    path: openNotePath,
    text: editorText,
  });
  const exportNote = exporter.exportNote;
  const exportCurrent = useCallback(() => {
    // FR-009: экспортировать можно только открытую заметку Typst.
    if (openNotePath !== null) void exportNote(openNotePath, editorText);
  }, [openNotePath, editorText, exportNote]);

  // Буфер редактора изменился — для автоснимка это изменение vault, хотя до
  // диска оно дойдёт через автосохранение чуть позже.
  useEffect(() => {
    syncRef.current?.noticeVaultChange();
  }, [editorText]);

  // Диалог конфликта — для активной вкладки. Конфликт по пути без вкладки (не
  // должен возникать: закрыть вкладку с правками нельзя) открывает вкладку.
  useEffect(() => setConflictFocus(activePath), [activePath]);
  const tryOpenedForConflict = useRef(new Set<string>());
  useEffect(() => {
    if (layout === null) return;
    const open = new Set(layout.groups.flatMap((group) => group.tabs.map((tab) => tab.path)));
    const pending = new Set(conflicts.state.conflicts.map((entry) => entry.notePath));
    for (const path of [...tryOpenedForConflict.current]) {
      if (!pending.has(path)) tryOpenedForConflict.current.delete(path);
    }
    for (const path of pending) {
      if (open.has(path) || tryOpenedForConflict.current.has(path)) continue;
      tryOpenedForConflict.current.add(path);
      void openTab(path);
    }
  }, [layout, conflicts.state.conflicts, openTab]);

  // ── Колонка заметки (фича 013) ───────────────────────────────────────────
  //
  // Запросы идут, только пока колонка видна (research.md §14): скрытая ничего
  // не читает. Целью служит открытая заметка или доска.
  // Порт правки открытых заметок (013 T047): «Связать» правит заметку в
  // редакторе, а не на диске. Активный редактор — через ref: порт создаётся
  // один раз на реестр.
  const activeEditorRef = useRef<() => { path: string; view: EditorView } | null>(() => null);
  activeEditorRef.current = () => {
    const view = getEditorView?.() ?? null;
    return view !== null && openNotePath !== null ? { path: openNotePath, view } : null;
  };
  const noteEditors = useMemo(
    () => plainEditors(registry, () => activeEditorRef.current()),
    [registry],
  );
  const links = useNoteLinks({
    client: state.client?.links ?? null,
    target: activePath,
    openTexts: registry?.openTexts() ?? NO_OPEN_TEXTS,
    enabled: view === "workspace" && notePanelOpen,
    revision: linksRevision,
    takeChanges: takeLinksChanges,
    bulkChanging: state.bulkChangeDepth > 0,
    mentionsExpanded: notePanel.mentionsExpanded,
    // Итог «Связать»: файл заметки-источника изменился.
    onLinked: (sourcePath) => bumpLinks([sourcePath]),
    editors: noteEditors,
  });
  const { backlinkCount, mentionCount } = links;
  const linksBadge = useMemo(() => {
    const parts: string[] = [];
    if (backlinkCount !== null) parts.push(formatNumber(backlinkCount.value));
    if (mentionCount !== null) parts.push(formatNumber(mentionCount));
    return parts.length === 0 ? null : parts.join(" · ");
  }, [backlinkCount, mentionCount, formatNumber]);
  // Сегмент статус-бара редактора — только при показанной колонке.
  const linksSummary = useMemo(() => {
    if (!notePanelOpen) return null;
    const parts: string[] = [];
    if (backlinkCount !== null) {
      parts.push(
        backlinkCount.unit === "links"
          ? t("links.status.links", { count: backlinkCount.value })
          : t("links.notesCount", { count: backlinkCount.value }),
      );
    }
    if (mentionCount !== null) parts.push(t("links.status.mentions", { count: mentionCount }));
    return parts.length === 0 ? null : parts.join(" · ");
  }, [notePanelOpen, backlinkCount, mentionCount, t]);

  // ── Оглавление заметки (фича 015) ──────────────────────────────────────
  //
  // Разбор идёт, только пока режим «Структура» виден (SC-004): скрытая колонка
  // ядро не зовёт. Оглавление есть только у заметки Typst (FR-012).
  const outlineShown = view === "workspace" && notePanelOpen && notePanel.mode === "outline";
  const outlineNotePath = active?.kind === "Note" ? openNotePath : null;
  const outlineKinds = notePanel.outlineKinds;
  const noteOutline = useOutline({
    client: state.client,
    notePath: outlineNotePath,
    text: editorText,
    kinds: outlineKinds,
    enabled: outlineShown,
  });
  const outlineTarget: OutlineTarget =
    active === null
      ? "none"
      : isBoardOpen
        ? "board"
        : typeof activeSnapshot.read === "object" && activeSnapshot.read.kind === "TooLarge"
          ? "tooLarge"
          : outlineNotePath !== null
            ? "note"
            : "none";
  const outlineCounts = noteOutline.view?.counts ?? null;
  const outlineBadge = useMemo(() => {
    if (outlineCounts === null) return null;
    const parts = [formatNumber(outlineCounts.headings)];
    if (outlineKinds.figures ?? false) {
      parts.push(formatNumber(outlineCounts.figures + outlineCounts.tables));
    }
    if (outlineKinds.labels ?? false) parts.push(formatNumber(outlineCounts.labels));
    return parts.join(" · ");
  }, [outlineCounts, outlineKinds, formatNumber]);
  const [headingRequest, setHeadingRequest] = useState<HeadingRequest | null>(null);

  // ── Свойства заметки и выборки (фича 020) ───────────────────────────────
  const propertiesActive = notePanelOpen && notePanel.mode === "properties";
  const noteProperties = useNoteProperties(
    state.client?.properties ?? null,
    openNotePath,
    editorText,
    view === "workspace" && propertiesActive,
  );
  const propertiesWorkspace = usePropertiesWorkspace({
    client: state.client,
    notePath: openNotePath,
    text: editorText,
    revision: linksRevision,
    getEditorView,
    onRebuild: () => setRebuildToken((token) => token + 1),
  });
  const pickTag = useCallback((tag: string) => {
    setSearchConditions([{ kind: "TagHas", tag }]);
    setNoteSheetOpen(false);
    setSearchOpen(true);
  }, []);

  // ── Синхронизация (фича 011) ─────────────────────────────────────────────
  //
  // Хук живёт здесь, а не в панели: индикатор виден и при закрытой панели, а
  // автоматика обязана идти, пока vault открыт.
  const [syncPanel, setSyncPanel] = useState<{ focus: "conflicts" | null } | null>(null);
  const [fileHistoryPath, setFileHistoryPath] = useState<string | null>(null);
  const sync = useSync(state.sync, state.client, t("sync.autoPrefix"), {
    // Файлы переписаны получением или откатом: открытая заметка обновляется
    // тем же путём, что и при внешнем изменении (001), дерево перечитывается.
    onFilesChanged: (paths) => {
      for (const path of paths) fileChanged(path);
      void refresh();
    },
  });
  syncRef.current = sync;

  // Пути с неразрешённым конфликтом — для значка в дереве. Список приходит
  // из ядра; перечитывается, когда меняется счётчик конфликтов сводки.
  const conflictCount = sync?.overview?.conflicts ?? 0;
  const syncClient = sync?.client ?? null;
  const syncRevision = sync?.revision ?? 0;
  const syncOverview = sync?.overview ?? null;
  const [conflictPaths, setConflictPaths] = useState<ReadonlySet<string>>(new Set());
  useEffect(() => {
    if (syncClient === null || conflictCount === 0) {
      setConflictPaths((current) => (current.size === 0 ? current : new Set()));
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const list = await syncClient.conflicts();
        if (!cancelled) setConflictPaths(new Set(list.map((conflict) => conflict.path)));
      } catch {
        // Значок — подсказка; сам список конфликтов покажет панель.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [syncClient, conflictCount, syncRevision]);

  const openFileHistory = useCallback((path: string) => setFileHistoryPath(path), []);
  const syncSnapshot = sync?.snapshot;
  const syncPush = sync?.push;
  const syncPull = sync?.pull;

  // Импорт из Obsidian (фича 021). Снимок после него предлагается, если история
  // включена (011): импортированное откатывается одним шагом.
  const importer = useImport(state.client, pickImportSource);
  const importSnapshot = syncOverview?.history.kind === "Local" ? syncSnapshot : undefined;
  const importTakeSnapshot = useMemo(
    () => (importSnapshot === undefined ? null : (message: string) => importSnapshot(null, message)),
    [importSnapshot],
  );

  // Возврат версии открытой заметки — правкой редактора, чтобы её отменял
  // обычный `Ctrl+Z` (FR-011). Для файла, не открытого в редакторе, версию
  // записывает ядро.
  const applyHistoryToEditor = useMemo(() => {
    if (fileHistoryPath === null || openNotePath !== fileHistoryPath || getEditorView === null) {
      return undefined;
    }
    return (text: string) => {
      const view = getEditorView();
      if (view === null) return;
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
        userEvent: "input",
      });
      view.focus();
    };
  }, [fileHistoryPath, openNotePath, getEditorView]);

  // Создание vault из удалённого репозитория (FR-013). Диалог один и на экране
  // приветствия, и поверх открытого vault.
  const [cloneOpen, setCloneOpen] = useState(false);
  const [cloneBusy, setCloneBusy] = useState(false);
  const [cloneError, setCloneError] = useState<SyncError | null>(null);
  const [cloneCapabilities, setCloneCapabilities] = useState<RemoteCapabilities | null>(null);
  const openClone = useMemo(
    () =>
      cloneVault === undefined
        ? null
        : () => {
            setCloneError(null);
            // Что оболочка умеет, отвечает ядро; до ответа диалог не показывается —
            // иначе подсказки под полями мигнули бы не теми словами.
            void cloneVault.capabilities().then((capabilities) => {
              setCloneCapabilities(capabilities);
              setCloneOpen(true);
            });
          },
    [cloneVault],
  );
  const submitClone = useCallback(
    async (request: CloneRequest) => {
      if (cloneVault === undefined) return;
      setCloneBusy(true);
      setCloneError(null);
      try {
        const opened = await cloneVault.run(request);
        // Выбор папки закрыли — диалог остаётся: адрес уже введён.
        if (!opened.ok && opened.error.kind === "PickerCancelled") return;
        setCloneOpen(false);
        setShowSettings(false);
        await openVaultWith(async () => opened);
      } catch (cause) {
        const failure = asSyncError(cause);
        setCloneError(failure.kind === "Cancelled" ? null : failure);
      } finally {
        setCloneBusy(false);
      }
    },
    [cloneVault, openVaultWith],
  );

  // FR-037: предупреждение перед закрытием окна с неразрешёнными правками.
  // Доски подчиняются тому же правилу: несохранённое на полотне удерживает
  // окно так же, как несохранённый текст (FR-011 фичи 002).
  useWindowCloseGuard(state.client);
  // Предупреждение об уходе с несохранённой доской держит её вкладка
  // (`BoardTab`), пока смонтирована; доска, чья запись при уходе не удалась,
  // удерживает окно и без вкладки.
  useEffect(() => {
    const warn = (event: BeforeUnloadEvent): void => {
      if (!boardControllers.hasFailed()) return;
      event.preventDefault();
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [boardControllers]);

  // Любое открытие файла — через вкладки (FR-001): новая вкладка, либо
  // активация уже открытой (FR-002). Дописывание доски при уходе с вкладки и
  // отказ уйти с недописанной делает хук вкладок.
  const openEntry = useCallback((next: Note) => void openTab(next.path), [openTab]);

  // Щелчок по узлу графа (фича 025): открыть заметку или доску и вернуться к рабочей области.
  const openPathFromGraph = useCallback(
    (path: string) => {
      void openTab(path);
      setView("workspace");
    },
    [openTab],
  );

  // Перенос в дереве (фича 023, FR-020): открытый файл остаётся открытым под
  // новым путём. Вкладки и документы переезжают сами — по событиям
  // `NoteRenamed`, которые ядро шлёт на каждый перенесённый файл (фича 014:
  // тот же документ под новым ключом, текст не перечитывается, каретка и
  // прокрутка остаются). Здесь за путём следуют только неразрешённые конфликты.
  const applyPathChanges = conflicts.pathsMoved;
  // «Переместить в…» из палитры — для открытого файла (FR-021). Окно живёт в
  // боковой панели рядом с остальным переносом; сюда уходит только запрос.
  const [moveToRequest, setMoveToRequest] = useState<{
    entry: { path: string; name: string };
    revision: number;
  } | null>(null);

  // Переход к месту в заметке: к ошибке экспорта и предпросмотра (009 FR-008,
  // FR-016) и к ссылке из колонки заметки (013 FR-003) — открыть файл и
  // поставить курсор на строку и колонку. Сам переход случается, когда файл уже
  // в редакторе, — см. эффект ниже.
  const revealPlace = useCallback(
    (place: RevealPlace) => {
      if (!state.notes.some((entry) => entry.path === place.path)) return;
      setExportOpen(false);
      // Переход из шторки закрывает её (макет 1c).
      setNoteSheetOpen(false);
      setView("workspace");
      const target: TextPlace = { line: place.line, column: place.column };
      void openTab(place.path, target);
    },
    [state.notes, openTab],
  );

  // Показ слайдов (фича 019). Состоянием показа владеет это окно: сборку
  // открытой заметки ему отдаёт панель предпросмотра (`onBuild`), каретку —
  // редактор, выход ставит каретку на показанный слайд тем же `revealPlace`,
  // что переход к ошибке.
  const [previewBuild, setPreviewBuild] = useState<PreviewSnapshot | null>(null);
  const noteBuild = useCallback((notePath: string, buildState: PreviewState) => {
    setPreviewBuild({ notePath, state: buildState });
  }, []);

  // ── Дополнение меток и ссылок (фича 018) ─────────────────────────────────
  // Метки связанных файлов, свидетельство последней удачной сборки, ключи
  // источников 017 и команды; сам список, отметки и переход — в редакторе.
  const linkedIndex = useLinkedIndex(previewClient, openNotePath, previewDependencies);
  const buildOfNote =
    previewBuild !== null && previewBuild.notePath === openNotePath ? previewBuild.state : null;
  const shownBuild = buildOfNote?.shown ?? null;
  const buildEvidence =
    shownBuild !== null && shownBuild.note_path === openNotePath ? (shownBuild.evidence ?? null) : null;
  const building = buildOfNote === null || buildOfNote.isBuilding || buildOfNote.isStale;
  const completionAuto = useCompletionAuto(device, showSettings);
  const [completionRequest, setCompletionRequest] = useState<{
    id: number;
    kind: "trigger" | "goto";
  } | null>(null);
  const requestCompletion = useCallback((kind: "trigger" | "goto") => {
    setCompletionRequest((previous) => ({ id: (previous?.id ?? 0) + 1, kind }));
  }, []);
  const triggerCompletion = useCallback(() => {
    if (openNotePath !== null) requestCompletion("trigger");
  }, [openNotePath, requestCompletion]);
  const gotoDefinition = useCallback(() => {
    if (openNotePath !== null) requestCompletion("goto");
  }, [openNotePath, requestCompletion]);
  // Все ключи источников — раз на изменение библиотеки, не на нажатие.
  const [sourceKeys, setSourceKeys] = useState<readonly string[]>([]);
  const sourcesClient = state.client?.sources ?? null;
  const sourcesHaveEntries = (sources.summary?.source_count ?? 0) > 0;
  useEffect(() => {
    if (sourcesClient === null || !sourcesHaveEntries) {
      setSourceKeys([]);
      return;
    }
    let alive = true;
    void sourcesClient
      .forCompletion("", 1_000_000)
      .then((hits) => {
        if (alive) setSourceKeys(hits.map((hit) => hit.source.key));
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [sourcesClient, sourcesHaveEntries, sources.revision]);
  const sourcesFor = useMemo(
    () =>
      sourcesClient === null || !sourcesHaveEntries
        ? null
        : async (query: string) =>
            (await sourcesClient.forCompletion(query, 50)).map((hit) => ({
              key: hit.source.key,
              summary: authorsLine(hit.source),
            })),
    [sourcesClient, sourcesHaveEntries],
  );
  const [sourcesFocus, setSourcesFocus] = useState<{ id: number; key: string } | null>(null);
  const onGoto = useCallback(
    (outcome: DefinitionOutcome) => {
      if (outcome.kind === "Place") {
        revealPlace({
          path: outcome.location.path,
          line: outcome.location.line,
          column: outcome.location.column,
        });
      } else if (outcome.kind === "Source") {
        // Источник показывается в колонке «Источники» (017).
        setSourcesFocus((previous) => ({ id: (previous?.id ?? 0) + 1, key: outcome.key }));
        if (narrow || drawer) {
          setNotePanelMode("sources");
          setNoteSheetOpen(true);
        } else {
          setNotePanelMode("sources");
          setNotePanelShown(true);
        }
      }
    },
    [revealPlace, narrow, drawer, setNotePanelMode, setNotePanelShown],
  );
  const createNoteAt = useCallback(
    async (path: string) => {
      const current = state.client;
      if (current === null) return false;
      try {
        await current.create(path);
        // Своя запись событий vault не публикует — отметки пересчитываются здесь.
        setNotesRevision((count) => count + 1);
        return true;
      } catch {
        return false;
      }
    },
    [state.client],
  );
  const noteCompletion = useMemo<NoteCompletion>(
    () => ({
      auto: completionAuto,
      linked: linkedIndex,
      evidence: buildEvidence,
      building,
      sourceKeys,
      sourcesFor,
      request: completionRequest,
      onGoto,
      createNote: createNoteAt,
      vaultRevision: notesRevision,
      narrow,
      readOnly: state.accessState === "readOnly",
    }),
    [
      completionAuto,
      linkedIndex,
      buildEvidence,
      building,
      sourceKeys,
      sourcesFor,
      completionRequest,
      onGoto,
      createNoteAt,
      notesRevision,
      narrow,
      state.accessState,
    ],
  );
  // Окно показа не открылось (блокировщик): тост с «Открыть ещё раз».
  const [showBlocked, setShowBlocked] = useState<{ retry: () => void } | null>(null);
  const showSession = useShowSession({
    host: showHost ?? null,
    slides: state.client?.slides ?? null,
    notePath: openNotePath,
    preview: previewBuild,
    caret: () => getEditorView?.()?.state.selection.main.head,
    locale: lang,
    reveal: (place) => {
      window.focus();
      revealPlace(place);
    },
    focusEditor: () => {
      window.focus();
      window.setTimeout(() => getEditorView?.()?.focus(), 0);
    },
    onBlocked: (retry) => setShowBlocked({ retry }),
  });
  const startShowSession = showSession.start;
  const startShow = useCallback(
    (from: "caret" | "first", withPresenter: boolean) => {
      setShowBlocked(null);
      void startShowSession(from, withPresenter);
    },
    [startShowSession],
  );

  // Перейти можно только к файлу, который редактор открывает, — то есть к
  // заметке из индекса vault.
  const canRevealPath = useCallback(
    (path: string) => state.notes.some((entry) => entry.path === path),
    [state.notes],
  );

  // Переход к пункту оглавления (FR-003, FR-004): каретка — тем же переходом,
  // что к ошибке, синхронно с жестом; предпросмотр догоняет отдельно и не
  // задерживает каретку. Из шторки `revealPlace` её закрывает.
  const focusEditor = useCallback(() => {
    getEditorView?.()?.focus();
  }, [getEditorView]);
  const outlineValue = noteOutline.outline;
  const jumpToOutlineRow = useCallback(
    (row: OutlineRowView) => {
      if (outlineNotePath === null) return;
      revealPlace({ path: outlineNotePath, line: row.place.line, column: row.place.column });
      if (outlineValue !== null && row.kind === "Heading") {
        const notePath = outlineNotePath;
        setHeadingRequest((previous) => ({
          notePath,
          outline: outlineValue,
          key: row.key,
          seq: (previous?.seq ?? 0) + 1,
        }));
      }
    },
    [outlineNotePath, outlineValue, revealPlace],
  );
  const closeNoteDrawer = useCallback(() => {
    setNoteSheetOpen(false);
    focusEditor();
  }, [focusEditor]);
  const outlineFilter = noteOutline.filter;
  const setOutlineFilter = noteOutline.setFilter;
  const escapeNoteDrawer = useCallback(() => {
    // Непустой фильтр Escape сперва очищает (макет 1b), шторку — следующий.
    if (notePanelMode === "outline" && outlineFilter !== "") setOutlineFilter("");
    else closeNoteDrawer();
  }, [notePanelMode, outlineFilter, setOutlineFilter, closeNoteDrawer]);

  const noteModes = useMemo<NotePanelModeSpec[]>(
    () => [
      {
        id: "links",
        title: "links.mode",
        badge: linksBadge,
        render: () => (
          <LinksMode
            // Другая заметка — другой разговор: раскрытые порции и выбранная
            // строка не переезжают с прежней.
            key={activePath ?? ""}
            hasTarget={active !== null}
            links={links}
            readOnly={state.accessState === "readOnly"}
            mentionsExpanded={notePanel.mentionsExpanded}
            onMentionsExpanded={notePanel.setMentionsExpanded}
            onReveal={revealPlace}
          />
        ),
      },
      {
        id: "outline",
        title: "outline.mode",
        badge: outlineBadge,
        render: () => (
          <OutlineMode
            target={outlineTarget}
            outline={noteOutline}
            kinds={outlineKinds}
            onKinds={notePanel.setOutlineKinds}
            onJump={jumpToOutlineRow}
            onFocusEditor={focusEditor}
            focusFilter={narrow || drawer}
          />
        ),
      },
      {
        id: "properties",
        title: "properties.mode",
        badge: null,
        render: () => (
          <PropertiesMode
            key={activePath ?? ""}
            hasTarget={active !== null && !isBoardOpen}
            note={activeNote}
            properties={noteProperties.properties}
            readOnly={state.accessState === "readOnly"}
            libraryCustomized={libraryCustomized}
            onEdit={propertiesWorkspace.applyEdit}
            onReveal={revealPlace}
            onPickTag={pickTag}
          />
        ),
      },
      {
        id: "graph",
        title: "graph.panelMode",
        badge: null,
        render: () => (
          <LocalGraphMode
            client={state.client}
            currentPath={activePath}
            settings={graphSettings.settings}
            depth={columnDepth}
            onDepthChange={setColumnDepth}
            onOpenPath={openPathFromGraph}
            onOpenAll={() => {
              setGraphScope("local");
              setGraphDepth(columnDepth);
              setView("graph");
            }}
          />
        ),
      },
      {
        id: "sources",
        title: "sources.mode",
        badge: sources.summary === null ? null : t("sources.badge", { count: sources.summary.source_count }),
        render: () =>
          state.client === null ? null : (
            <SourcesPanel
              client={state.client}
              summary={sources.summary}
              scan={sources.scan}
              revision={sources.revision}
              readOnly={sourcesReadOnly}
              uncitedOnly={notePanel.sourcesUncitedOnly}
              onUncitedOnly={notePanel.setSourcesUncitedOnly}
              onStartScan={sources.startScan}
              onCancelScan={sources.cancelScan}
              onReveal={revealPlace}
              onInsert={openNotePath === null ? null : requestCitation}
              onCreateFile={createSourceFileFromPanel}
              onRegisterFile={registerSourceFileFromPanel}
              focus={sourcesFocus}
            />
          ),
      },
    ],
    [
      linksBadge,
      activePath,
      active,
      links,
      state.accessState,
      notePanel.mentionsExpanded,
      notePanel.setMentionsExpanded,
      revealPlace,
      activeNote,
      isBoardOpen,
      noteProperties.properties,
      libraryCustomized,
      propertiesWorkspace.applyEdit,
      pickTag,
      state.client,
      graphSettings.settings,
      columnDepth,
      openPathFromGraph,
      outlineBadge,
      outlineTarget,
      noteOutline,
      outlineKinds,
      notePanel.setOutlineKinds,
      jumpToOutlineRow,
      focusEditor,
      narrow,
      drawer,
      sources.summary,
      sources.scan,
      sources.revision,
      sources.startScan,
      sources.cancelScan,
      sourcesReadOnly,
      notePanel.sourcesUncitedOnly,
      notePanel.setSourcesUncitedOnly,
      openNotePath,
      requestCitation,
      createSourceFileFromPanel,
      registerSourceFileFromPanel,
      sourcesFocus,
      t,
    ],
  );

  /**
   * Версия, принятая при разрешении конфликта (FR-022): документ заметки
   * получает её как уже сохранённую и разносит по своим видам.
   *
   * Записывать её обратно нельзя: она уже на диске — ядро её оттуда и прочитало, —
   * а буфер, оставленный после «принять внешнюю», ушёл бы в файл сам и отменил
   * только что сделанный выбор пользователя.
   */
  const handleAcceptExternal = useCallback(async () => {
    // Путь спрашивается до разрешения: после него конфликт снят и текущего
    // конфликта уже нет.
    const path = conflicts.current?.notePath ?? null;
    const external = await conflicts.acceptExternal();
    if (path === null) return;
    // `null` — внешней версии нет: заметку удалили снаружи (FR-025).
    if (external === null) void dispatchTab({ kind: "Deleted", path });
    else registry?.get(path)?.replace(external, true);
  }, [conflicts, registry, dispatchTab]);

  // Доски vault для выбора при вставке (FR-018). Тот же список, что в дереве:
  // отдельного запроса к ядру за досками нет, вид спрашивается у ядра.
  const boards = useMemo(
    () =>
      state.notes
        .filter((note) => isBoard(note))
        .map((note) => ({ path: note.path, name: note.name })),
    [state.notes],
  );

  /**
   * Открыть файл по пути. Список берётся у ядра заново, а не из `state.notes`:
   * только что созданный файл в нём ещё не появился.
   */
  const openPath = useCallback(async (path: string) => openTab(path), [openTab]);

  // Отчёт импорта (фича 021) записан ядром, но список заметок узнаёт о нём по
  // событию чуть позже: открытие ждёт его появления (до 3 с), иначе вкладка не откроется.
  const notesRef = useRef(state.notes);
  notesRef.current = state.notes;
  const openImportReport = useCallback(
    async (path: string) => {
      for (
        let waited = 0;
        waited < 30 && !notesRef.current.some((note) => note.path === path);
        waited += 1
      ) {
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      await openPath(path);
    },
    [openPath],
  );

  /** Переход от вставки в тексте к её доске на полотне (FR-025). */
  const openBoard = useCallback((boardPath: string) => void openTab(boardPath), [openTab]);

  // ── Быстрый захват и «Сегодня» (фича 022) ────────────────────────────────
  //
  // Правила — в ядре; здесь только открытие заметки, правка буфера открытой
  // заметки и показ листа. «Сегодня» и «Во входящие» работают во всех оболочках,
  // приём «Поделиться» — только если оболочка дала `captureSource`.
  // Каретка в конец и фокус редактору. Свежесозданная заметка приходит событиями
  // vault уже после открытия, и редактор успевает пересоздаться: поэтому
  // просьба живёт недолго и исполняется на каждый новый редактор этой заметки.
  const [endFocus, setEndFocus] = useState<string | null>(null);
  useEffect(() => {
    if (endFocus === null) return undefined;
    const timer = window.setTimeout(() => setEndFocus(null), 1500);
    return () => window.clearTimeout(timer);
  }, [endFocus]);
  const activeRead = activeSnapshot.read;
  const getEditorViewRef = useRef(getEditorView);
  getEditorViewRef.current = getEditorView;
  useEffect(() => {
    if (endFocus === null || endFocus !== openNotePath || activeRead !== "ready") return undefined;
    const settle = (): void => {
      const view = getEditorViewRef.current?.() ?? null;
      if (view === null || view.hasFocus) return;
      view.dispatch({ selection: { anchor: view.state.doc.length }, scrollIntoView: true });
      view.focus();
    };
    settle();
    const interval = window.setInterval(settle, 80);
    // Пользователь взялся за мышь или клавиатуру сам — больше не вмешиваемся.
    const release = (): void => setEndFocus(null);
    window.addEventListener("pointerdown", release, true);
    window.addEventListener("keydown", release, true);
    return () => {
      window.clearInterval(interval);
      window.removeEventListener("pointerdown", release, true);
      window.removeEventListener("keydown", release, true);
    };
  }, [endFocus, openNotePath, activeRead]);
  const openAtEnd = useCallback(
    async (path: string) => {
      setEndFocus(path);
      setExportOpen(false);
      setNoteSheetOpen(false);
      setView("workspace");
      await openTab(path, END_OF_NOTE);
    },
    [openTab],
  );
  const daily = useDailyNote({
    client: state.client,
    overrides: vaultSettings.overrides,
    activePath: openNotePath,
    revision: linksRevision,
    openAtEnd,
    onCreated: () => void refresh(),
    notify: contextMenu.notify,
    t,
  });
  const recentNotes = useMemo(
    () => [
      ...new Set(
        (layout?.groups ?? [])
          .flatMap((group) => group.tabs)
          .filter((tab) => tab.kind !== "Board")
          .map((tab) => tab.path),
      ),
    ],
    [layout],
  );
  const capture = useCapture({
    client: state.client,
    vaultOpening: state.isLoading,
    source: captureSource,
    openTextOf: (path) => {
      const document = registry?.get(path);
      return document !== undefined && document.snapshot().read === "ready"
        ? document.text()
        : undefined;
    },
    applyEdit: async (path, edit, expected) => {
      const document = registry?.get(path);
      if (document === undefined || document.text() !== expected) return false;
      const view = path === openNotePath ? (getEditorView?.() ?? null) : null;
      if (view !== null && view.state.doc.toString() === expected) {
        // Открытая в редакторе заметка получает захват правкой, которую можно
        // отменить, а сохраняется обычным автосохранением (FR-004).
        view.dispatch({
          changes: { from: edit.from, to: edit.to, insert: edit.insert },
          userEvent: "input",
        });
      } else {
        document.replace(`${expected.slice(0, edit.from)}${edit.insert}${expected.slice(edit.to)}`, false);
      }
      await document.flush();
      return true;
    },
    recent: recentNotes,
    onWritten: () => void refresh(),
    onToday: () => void daily.openToday(),
  });
  const dailyPosition = daily.position;
  const dailyOpenToday = daily.openToday;
  const dailyOpenPrevious = daily.openPrevious;
  const dailyOpenNext = daily.openNext;
  const openManualCapture = capture.openManual;
  const captureSheet = capture.sheet !== null && (
    <CaptureSheet model={capture.sheet} vaultName={state.client?.rootName ?? null} />
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
      await openPath(board.path);
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
    const record = state.notes.find((note) => note.path === path);
    return record && isBoard(record) ? path : null;
  }, [conflicts, state.notes]);
  const boardVersions = useBoardConflict(state.client, boardConflictPath);

  const handleAcceptExternalBoard = useCallback(async () => {
    const path = boardConflictPath;
    const external = await conflicts.acceptExternal();
    if (path === null) return;
    // Внешней версии нет — доску удалили снаружи: показывать полотно нечем.
    if (external === null) void dispatchTab({ kind: "Deleted", path });
    else boardControllers.replace(path, external);
  }, [conflicts, boardConflictPath, dispatchTab, boardControllers]);

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

  // Уход с рабочей области дописывает доску: её вкладка размонтируется и сама
  // сбрасывает дебаунс автосохранения (`BoardTab`).
  const toggleView = useCallback(() => {
    setView((current) => (current === "workspace" ? "graph" : "workspace"));
  }, []);

  // ── Команды вкладок (фича 014) ───────────────────────────────────────────
  //
  // Что команда значит, решает ядро: здесь только выбор действия по показу.
  // Показ читается из ссылки, чтобы слушатель клавиш не пересоздавался на каждый
  // переход.
  const layoutRef = useRef(layout);
  layoutRef.current = layout;
  const tabCommands = useMemo(() => {
    const activeTabId = (): number | null => layoutRef.current?.active?.tab ?? null;
    return {
      close: () => {
        const tab = activeTabId();
        if (tab !== null) void tabs.close(tab);
      },
      closeOthers: () => {
        const tab = activeTabId();
        if (tab !== null) void tabs.closeOthers(tab);
      },
      next: () => void dispatchTab({ kind: "Cycle", direction: "Next" }),
      previous: () => void dispatchTab({ kind: "Cycle", direction: "Previous" }),
      back: () => void dispatchTab({ kind: "Back" }),
      forward: () => void dispatchTab({ kind: "Forward" }),
      toggleSplit: () =>
        void dispatchTab(layoutRef.current?.groups.length === 2 ? { kind: "Unsplit" } : { kind: "Split" }),
      moveToOther: () => {
        const tab = activeTabId();
        if (tab !== null) void dispatchTab({ kind: "MoveToOtherGroup", tab });
      },
      focusOther: () => {
        const current = layoutRef.current;
        if (current === null || current.groups.length < 2) return;
        void dispatchTab({
          kind: "FocusGroup",
          group: current.active_group === "First" ? "Second" : "First",
        });
      },
      moveLeft: () => {
        const tab = activeTabId();
        if (tab !== null) void tabs.moveBy(tab, -1);
      },
      moveRight: () => {
        const tab = activeTabId();
        if (tab !== null) void tabs.moveBy(tab, 1);
      },
    };
    // `tabs.close` и подобные стабильны (useCallback без меняющихся зависимостей).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dispatchTab, tabs.close, tabs.closeOthers, tabs.moveBy]);

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
      } else if (shortcuts.matches("toggle-note-panel", event)) {
        event.preventDefault();
        toggleNotePanel();
      } else if (shortcuts.matches("toggle-outline", event)) {
        event.preventDefault();
        toggleOutline();
      } else if (shortcuts.matches("toggle-sources", event)) {
        event.preventDefault();
        toggleSources();
      } else if (shortcuts.matches("insert-citation", event)) {
        event.preventDefault();
        insertCitationCommand();
      } else if (shortcuts.matches("goto-definition", event)) {
        event.preventDefault();
        gotoDefinition();
      } else if (shortcuts.matches("trigger-completion", event)) {
        event.preventDefault();
        triggerCompletion();
      } else if (shortcuts.matches("tab-close", event)) {
        event.preventDefault();
        tabCommands.close();
      } else if (shortcuts.matches("tab-next", event)) {
        event.preventDefault();
        tabCommands.next();
      } else if (shortcuts.matches("tab-previous", event)) {
        event.preventDefault();
        tabCommands.previous();
      } else if (shortcuts.matches("split-editor", event)) {
        event.preventDefault();
        tabCommands.toggleSplit();
      } else if (shortcuts.matches("tab-move-to-other-group", event)) {
        event.preventDefault();
        tabCommands.moveToOther();
      } else if (shortcuts.matches("navigate-back", event)) {
        event.preventDefault();
        tabCommands.back();
      } else if (shortcuts.matches("navigate-forward", event)) {
        event.preventDefault();
        tabCommands.forward();
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
      } else if (shortcuts.matches("insert-image", event)) {
        // Фича 016: изображение из файла к каретке открытой заметки.
        event.preventDefault();
        insertImage();
      } else if (shortcuts.matches("today", event)) {
        // Фича 022: ежедневная заметка на сегодня.
        event.preventDefault();
        void daily.openToday();
      } else if (showHost !== undefined && shortcuts.matches("start-show", event)) {
        // Показ слайдов (019): «с текущего слайда» у открытой заметки.
        event.preventDefault();
        if (view === "workspace" && openNotePath !== null) startShow("caret", false);
      } else if (syncClient !== null && shortcuts.matches("sync-open-panel", event)) {
        event.preventDefault();
        setSyncPanel((open) => (open === null ? { focus: null } : null));
      } else if (syncClient !== null && shortcuts.matches("sync-snapshot", event)) {
        event.preventDefault();
        void syncSnapshot?.(null, "");
      } else if (syncClient !== null && shortcuts.matches("sync-file-history", event)) {
        event.preventDefault();
        if (openNotePath !== null) setFileHistoryPath(openNotePath);
      } else if (syncClient !== null && shortcuts.matches("sync-push", event)) {
        event.preventDefault();
        void syncPush?.();
      } else if (syncClient !== null && shortcuts.matches("sync-pull", event)) {
        event.preventDefault();
        void syncPull?.();
      } else if (syncClient !== null && shortcuts.matches("sync-resolve-conflicts", event)) {
        event.preventDefault();
        setSyncPanel({ focus: "conflicts" });
      }
      // Escape здесь больше не разбирается: закрытием наложений занят
      // `useDismissOnEscape`, и закрывает он **верхнее** из открытых. Отдельная
      // ветка тут закрывала бы палитру заодно с диалогом поверх неё.
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [
    shortcuts,
    toggleView,
    toggleNotePanel,
    toggleOutline,
    toggleSources,
    insertCitationCommand,
    gotoDefinition,
    triggerCompletion,
    tabCommands,
    exportCurrent,
    syncClient,
    syncSnapshot,
    syncPush,
    syncPull,
    openNotePath,
    daily.openToday,
    insertImage,
    showHost,
    startShow,
    view,
  ]);

  /**
   * Команды палитры. Только те, что действительно что-то делают: список из
   * макета содержит экспорт BibTeX и переключение vim-режима, которых в
   * приложении нет, а мёртвая строка в палитре хуже её отсутствия.
   */
  // Ссылка на заметку из контекстного меню дерева — в позицию курсора открытой
  // заметки. Путь от корня vault (`[[/путь]]`): так ссылка не зависит от папки,
  // в которой лежит открытая заметка (`graph::resolve_link_target`).
  const insertLink = useMemo(() => {
    if (getEditorView === null || openNotePath === null) return undefined;
    return (target: Note) => {
      const view = getEditorView();
      if (view === null) return;
      view.dispatch(view.state.replaceSelection(`[[/${target.path}]]`), {
        userEvent: "input",
        scrollIntoView: true,
      });
      view.focus();
    };
  }, [getEditorView, openNotePath]);

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
        id: "note-panel",
        ...titled("notePanel.command"),
        icon: "link",
        meta: shortcuts.displayOf("toggle-note-panel"),
        run: toggleNotePanel,
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
    if (view === "workspace" && openNotePath !== null) {
      items.push({
        id: "element-params",
        ...titled("workspace.elementParams"),
        icon: "terminal",
        meta: shortcuts.displayOf("element-params"),
        run: () => setParamsRequest((request) => request + 1),
      });
    }
    // Фича 015. Оглавление — только у открытой заметки Typst: у доски и
    // прочих файлов его нет (FR-012).
    if (view === "workspace" && outlineNotePath !== null) {
      items.push({
        id: "toggle-outline",
        ...titled("outline.button"),
        icon: "list",
        meta: shortcuts.displayOf("toggle-outline"),
        run: toggleOutline,
      });
    }
    // Фича 017. «Источники» — при открытом vault; «Вставить цитату…» — у
    // открытой заметки Typst.
    if (view === "workspace") {
      items.push({
        id: "toggle-sources",
        ...titled("sources.command.toggle"),
        icon: "list",
        meta: shortcuts.displayOf("toggle-sources"),
        run: toggleSources,
      });
    }
    if (view === "workspace" && outlineNotePath !== null) {
      items.push({
        id: "insert-citation",
        ...titled("sources.command.insertCitation"),
        icon: "plus",
        meta: shortcuts.displayOf("insert-citation"),
        run: insertCitationCommand,
      });
    }
    // Фича 018. Переход и ручной вызов дополнения — у открытой заметки Typst.
    if (view === "workspace" && outlineNotePath !== null) {
      items.push(
        {
          id: "goto-definition",
          ...titled("completion.command.goto"),
          icon: "link",
          meta: shortcuts.displayOf("goto-definition"),
          run: gotoDefinition,
        },
        {
          id: "trigger-completion",
          ...titled("completion.command.trigger"),
          icon: "list",
          meta: shortcuts.displayOf("trigger-completion"),
          run: triggerCompletion,
        },
      );
    }
    // Фича 019. Показ слайдов — только у открытой заметки Typst и там, где
    // оболочка умеет открывать окно показа.
    if (view === "workspace" && openNotePath !== null && showHost !== undefined) {
      items.push(
        {
          id: "show-current",
          ...titled("slides.command.fromCurrent"),
          icon: "play",
          meta: shortcuts.displayOf("start-show"),
          run: () => startShow("caret", false),
        },
        {
          id: "show-first",
          ...titled("slides.command.fromStart"),
          icon: "start",
          run: () => startShow("first", false),
        },
      );
      if (showHost.supports.presenter) {
        items.push({
          id: "show-presenter",
          ...titled("slides.command.withPresenter"),
          icon: "presenter",
          run: () => startShow("caret", true),
        });
      }
    }
    // Фича 016. «Вставить изображение…» — у открытой заметки Typst; в vault
    // только на чтение пачка отвечает отказом с причиной (FR-024).
    if (view === "workspace" && outlineNotePath !== null) {
      items.push({
        id: "insert-image",
        ...titled("attachments.command.insertImage"),
        icon: "plus",
        meta: shortcuts.displayOf("insert-image"),
        run: insertImage,
      });
    }
    // Фича 022. «Сегодня» и «Во входящие» — во всех оболочках; соседние дни —
    // у открытой ежедневной заметки, и только в ту сторону, где заметка есть.
    if (state.client !== null) {
      items.push(
        {
          id: "capture-today",
          ...titled("capture.command.today"),
          icon: "page",
          meta: shortcuts.displayOf("today"),
          run: () => void dailyOpenToday(),
        },
        {
          id: "capture-inbox",
          ...titled("capture.command.inbox"),
          icon: "plus",
          run: openManualCapture,
        },
      );
      if (view === "workspace" && dailyPosition?.previous != null) {
        items.push({
          id: "daily-previous",
          ...titled("capture.command.previousDay"),
          icon: "chevron",
          run: () => void dailyOpenPrevious(),
        });
      }
      if (view === "workspace" && dailyPosition?.next != null) {
        items.push({
          id: "daily-next",
          ...titled("capture.command.nextDay"),
          icon: "chevron",
          run: () => void dailyOpenNext(),
        });
      }
    }
    // Фича 014. Команды вкладок: каждая доступна, только если ядро говорит, что
    // действие возможно (FR-020); без сочетания по умолчанию — «закрыть остальные»,
    // «передвинуть» и «фокус в другую группу» (research.md §12).
    if (view === "workspace" && layout !== null) {
      const activeGroup = layout.groups.find((group) => group.slot === layout.active_group);
      const hasActive = layout.active !== undefined;
      const split = layout.groups.length === 2;
      if (hasActive) {
        items.push({
          id: "tab-close",
          ...titled("workspace.tabs.close"),
          icon: "x",
          meta: shortcuts.displayOf("tab-close"),
          run: tabCommands.close,
        });
        if ((activeGroup?.tabs.length ?? 0) > 1 || layout.tab_count > 1) {
          items.push(
            {
              id: "tab-close-others",
              ...titled("workspace.tabs.closeOthers"),
              icon: "x",
              run: tabCommands.closeOthers,
            },
            {
              id: "tab-next",
              ...titled("workspace.tabs.next"),
              icon: "tab",
              meta: shortcuts.displayOf("tab-next"),
              run: tabCommands.next,
            },
            {
              id: "tab-previous",
              ...titled("workspace.tabs.previous"),
              icon: "tab",
              meta: shortcuts.displayOf("tab-previous"),
              run: tabCommands.previous,
            },
            {
              id: "tab-move-left",
              ...titled("workspace.tabs.moveLeft"),
              icon: "tab",
              run: tabCommands.moveLeft,
            },
            {
              id: "tab-move-right",
              ...titled("workspace.tabs.moveRight"),
              icon: "tab",
              run: tabCommands.moveRight,
            },
          );
        }
      }
      if (activeGroup?.can_back === true) {
        items.push({
          id: "navigate-back",
          ...titled("workspace.tabs.back"),
          icon: "chevron",
          meta: shortcuts.displayOf("navigate-back"),
          run: tabCommands.back,
        });
      }
      if (activeGroup?.can_forward === true) {
        items.push({
          id: "navigate-forward",
          ...titled("workspace.tabs.forward"),
          icon: "chevron",
          meta: shortcuts.displayOf("navigate-forward"),
          run: tabCommands.forward,
        });
      }
      if (split ? layout.can_unsplit : layout.can_split) {
        items.push({
          id: "split-editor",
          ...titled(split ? "workspace.tabs.unsplit" : "workspace.tabs.split"),
          icon: "layout",
          meta: shortcuts.displayOf("split-editor"),
          run: tabCommands.toggleSplit,
        });
      }
      if (layout.can_move_to_other) {
        items.push({
          id: "tab-move-to-other-group",
          ...titled("workspace.tabs.moveToOther"),
          icon: "layout",
          meta: shortcuts.displayOf("tab-move-to-other-group"),
          run: tabCommands.moveToOther,
        });
      }
      if (split) {
        items.push({
          id: "focus-other-group",
          ...titled("workspace.tabs.focusOther"),
          icon: "layout",
          run: tabCommands.focusOther,
        });
      }
    }
    // Фича 023, FR-021. «Переместить в…» — для открытого файла, заметки или
    // доски. Окно выбора папки живёт в боковой панели, поэтому скрытая панель
    // сначала показывается.
    if (view === "workspace" && activeNote !== null) {
      const entry = { path: activeNote.path, name: activeNote.name };
      items.push({
        id: "move-entry",
        ...titled("tree.move.menu"),
        icon: "folder",
        run: () => {
          if (sidebarHidden) showSidebar();
          setMoveToRequest((previous) => ({ entry, revision: (previous?.revision ?? 0) + 1 }));
        },
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
    // Фича 021. Команда видна и там, где папку выбрать нельзя: диалог называет
    // причину (C5). Нужен открытый vault и не идущий сейчас импорт.
    if (importer.offered && state.client !== null) {
      items.push({
        id: "import-obsidian",
        ...titled("import.command"),
        icon: "folder",
        run: importer.openDialog,
      });
    }
    // Фича 011. Команды синхронизации — пока vault открыт с ней. Обмен и
    // история заметки предлагаются только там, где они возможны: мёртвая
    // строка в палитре хуже её отсутствия; почему действие недоступно,
    // объясняет панель (FR-036).
    if (syncClient !== null) {
      const history = syncOverview?.history.kind ?? null;
      items.push({
        id: "sync-open-panel",
        ...titled("sync.command.openPanel"),
        icon: "refresh",
        meta: shortcuts.displayOf("sync-open-panel"),
        run: () => setSyncPanel({ focus: null }),
      });
      if (history === "Local") {
        items.push({
          id: "sync-snapshot",
          ...titled("sync.command.snapshot"),
          icon: "check",
          meta: shortcuts.displayOf("sync-snapshot"),
          run: () => void syncSnapshot?.(null, ""),
        });
      }
      if ((history === "Local" || history === "Mirror") && openNotePath !== null) {
        items.push({
          id: "sync-file-history",
          ...titled("sync.command.fileHistory"),
          icon: "list",
          meta: shortcuts.displayOf("sync-file-history"),
          run: () => setFileHistoryPath(openNotePath),
        });
      }
      if (history === "Local" && syncOverview?.remote !== undefined) {
        items.push(
          {
            id: "sync-push",
            ...titled("sync.command.push"),
            icon: "download",
            meta: shortcuts.displayOf("sync-push"),
            run: () => void syncPush?.(),
          },
          {
            id: "sync-pull",
            ...titled("sync.command.pull"),
            icon: "download",
            meta: shortcuts.displayOf("sync-pull"),
            run: () => void syncPull?.(),
          },
        );
      }
      if ((syncOverview?.conflicts ?? 0) > 0) {
        items.push({
          id: "sync-resolve-conflicts",
          ...titled("sync.command.resolveConflicts"),
          icon: "refresh",
          meta: shortcuts.displayOf("sync-resolve-conflicts"),
          run: () => setSyncPanel({ focus: "conflicts" }),
        });
      }
    }
    if (openClone !== null) {
      items.push({
        id: "sync-clone",
        ...titled("sync.command.clone"),
        icon: "download",
        run: openClone,
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
    insertImage,
    view,
    toggleView,
    toggleNotePanel,
    toggleOutline,
    toggleSources,
    insertCitationCommand,
    gotoDefinition,
    triggerCompletion,
    outlineNotePath,
    createBoard,
    settings,
    shortcuts,
    layout,
    tabCommands,
    previewClient,
    openNotePath,
    activeNote,
    sidebarHidden,
    showSidebar,
    exportCurrent,
    showHost,
    startShow,
    syncClient,
    syncOverview,
    syncSnapshot,
    syncPush,
    syncPull,
    openClone,
    importer.offered,
    importer.openDialog,
    state.client,
    dailyPosition,
    dailyOpenToday,
    dailyOpenPrevious,
    dailyOpenNext,
    openManualCapture,
    t,
    tEn,
  ]);

  const cloneDialog = cloneOpen && cloneVault !== undefined && cloneCapabilities !== null && (
    <RemoteDialog
      mode="clone"
      capabilities={cloneCapabilities}
      busy={cloneBusy}
      error={cloneError}
      onSubmit={(request) => void submitClone(request)}
      onCancel={() => {
        if (cloneBusy) cloneVault.cancel();
        else setCloneOpen(false);
      }}
    />
  );

  if (!state.client) {
    return (
      <>
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
            onClone={openClone ?? undefined}
          />
          {cloneDialog}
        </main>
        {/* Лист захвата без vault (макет D3) стоит рядом с экраном приветствия, а не
            внутри: стили приветствия окрасили бы его текст. */}
        {captureSheet !== false && <IconSprite />}
        {captureSheet}
      </>
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
          const path = activePath;
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
          <Breadcrumb rootName={state.client.rootName} path={activePath} />
          {/* Телефон (макет 1c): «Назад», «Вперёд» и название открытой заметки —
              вместо полосы вкладок. Шапка уже скрывает крошки. */}
          {layout !== null && layout.presentation === "List" && view === "workspace" && (
            <PhoneNav
              layout={layout}
              onBack={tabCommands.back}
              onForward={tabCommands.forward}
              backHint={hintWith(t("workspace.tabs.back"), shortcuts.displayOf("navigate-back"))}
              forwardHint={hintWith(t("workspace.tabs.forward"), shortcuts.displayOf("navigate-forward"))}
            />
          )}
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

          {/* Ход импорта из Obsidian, свёрнутый из карточки (021, C2). */}
          {importer.run !== null && importer.running && importer.run.minimized && (
            <button
              type="button"
              className="ts-export-pill"
              title={t("import.progress.expand")}
              onClick={importer.toggleMinimized}
            >
              <span className="ts-hazard" aria-hidden="true">
                <span className="ts-hazard__fill" />
              </span>
              {t("import.pill", {
                done: importer.run.state?.next ?? 0,
                total: importer.run.state?.total ?? 0,
              })}
            </button>
          )}

          {/* Индикатор синхронизации (011 FR-035): в шапке, а не в статус-баре
              редактора — того нет при открытой доске и в графе, а состояние
              обмена должно быть видно всегда. */}
          {sync !== null && (
            <SyncIndicator
              sync={sync}
              panelOpen={syncPanel !== null}
              onOpenPanel={() =>
                setSyncPanel((open) => (open === null ? { focus: null } : null))
              }
            />
          )}

          <span className="ts-divider" />

          {/* Телефон: счётчик открытых заметок раскрывает их список (макет 1c). */}
          {layout !== null && layout.presentation === "List" && view === "workspace" && (
            <button
              type="button"
              className="ts-btn ts-open-notes__count"
              aria-pressed={openNotesSheet}
              aria-label={t("workspace.tabs.openNotesButton", { count: layout.tab_count })}
              title={t("workspace.tabs.openNotes")}
              onClick={() => setOpenNotesSheet((open) => !open)}
            >
              <Icon name="tab" size={14} />
              {formatNumber(layout.tab_count)}
            </button>
          )}

          {/* «Сегодня» и ‹ › у ежедневной заметки (022, макет D4). */}
          {view === "workspace" && (
            <DailyNav daily={daily} todayShortcut={shortcuts.displayOf("today")} />
          )}

          {/* Показ слайдов (019, макет A1): только у открытой заметки Typst. */}
          {view === "workspace" && showHost !== undefined && openNotePath !== null && (
            <ShowMenu
              canPresent={showHost.supports.presenter}
              shortcut={shortcuts.displayOf("start-show")}
              active={showSession.active}
              onStart={startShow}
            />
          )}

          {/* Оглавление (015, макет 1a): та же колонка в режиме «Структура». */}
          {view === "workspace" && (
            <button
              type="button"
              className="ts-btn"
              aria-pressed={outlineShown}
              title={hintWith(t("outline.button"), shortcuts.displayOf("toggle-outline"))}
              onClick={toggleOutline}
            >
              {t("outline.button")}
            </button>
          )}

          {/* Источники (017, макет 1a): та же колонка в режиме «Источники». */}
          {view === "workspace" && (
            <button
              type="button"
              className="ts-btn"
              aria-pressed={notePanelOpen && notePanel.mode === "sources"}
              title={hintWith(t("sources.command.toggle"), shortcuts.displayOf("toggle-sources"))}
              onClick={toggleSources}
            >
              {t("sources.command.toggle")}
            </button>
          )}

          {/* Колонка заметки (013): на узком окне кнопка подписана названием
              активного режима и открывает шторку. */}
          {view === "workspace" && (
            <button
              type="button"
              className="ts-btn"
              aria-pressed={notePanelOpen}
              title={shortcuts.displayOf("toggle-note-panel") || undefined}
              onClick={toggleNotePanel}
            >
              {narrow
                ? t(noteModes.find((mode) => mode.id === notePanel.mode)?.title ?? "links.mode")
                : t("notePanel.button")}
            </button>
          )}

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
        {showBlocked !== null && (
          <div role="alert">
            <p>{t("slides.blocked")}</p>
            <button
              type="button"
              className="ts-btn"
              onClick={() => {
                const { retry } = showBlocked;
                setShowBlocked(null);
                retry();
              }}
            >
              {t("slides.reopen")}
            </button>
            <button
              type="button"
              className="ts-btn ts-btn--ghost"
              aria-label={t("slides.dismiss")}
              onClick={() => setShowBlocked(null)}
            >
              <Icon name="x" size={12} />
            </button>
          </div>
        )}
        {attachments.notice !== null && (
          <AttachmentNotices
            notice={attachments.notice}
            onCancel={attachments.cancel}
            onDismiss={attachments.dismiss}
            onOpenRules={sync === null ? null : setRulesFile}
          />
        )}
        {exporter.last !== null && (
          <ExportToast
            result={exporter.last}
            onDismiss={exporter.dismissLast}
            onReveal={revealPlace}
            onOpenCenter={() => setExportOpen(true)}
          />
        )}
      </div>

      {view === "graph" ? (
        <GraphView
          client={state.client}
          currentPath={activePath}
          settings={graphSettings}
          scope={graphScope}
          onScopeChange={setGraphScope}
          depth={graphDepth}
          onDepthChange={setGraphDepth}
          onOpenPath={openPathFromGraph}
          onBack={toggleView}
        />
      ) : (
      <>
      <div
        className="ts-body"
        ref={panes.bodyRef}
        style={panes.style}
        {...(phone ? { "data-phone-pane": shownPhonePane } : {})}
      >
        {/* Скрытое дерево не рендерится вовсе, а не сжимается до нуля: панель
            нулевой ширины оставляла бы в раскладке свою рамку и по-прежнему
            читала бы vault ради счётчика в подвале. */}
        {(!panes.sidebarHidden || phone) && (
          <>
            <Sidebar
              client={state.client}
              notes={state.notes}
              openPath={activePath}
              folderCount={folderCount}
              onOpenNote={openEntry}
              onNotesChanged={refresh}
              onPathsMoved={applyPathChanges}
              moveToRequest={moveToRequest}
              onMoveToRequestHandled={() => setMoveToRequest(null)}
              onCreated={(path) => void openPath(path)}
              onError={setOperationError}
              onOpenGraph={toggleView}
              templates={templates}
              onOpenTemplates={() => setTemplatesOpen(true)}
              createFromTemplate={createFromTemplate}
              shortcuts={shortcuts}
              onInsertLink={insertLink}
              onFileHistory={
                syncOverview?.history.kind === "Local" || syncOverview?.history.kind === "Mirror"
                  ? (note) => openFileHistory(note.path)
                  : undefined
              }
              conflictPaths={conflictPaths}
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

        {/* Колонка заметки (013): между деревом и редактором, а при открытой
            доске — рядом с контейнером полотна, в его DOM не входя. На узком
            окне вместо неё шторка (ниже). */}
        {!narrow && !drawer && notePanel.shown && (
          <NotePanel
            modes={noteModes}
            active={notePanel.mode}
            onSelect={notePanel.setMode}
            presentation="column"
            onClose={notePanel.toggle}
          />
        )}

        {/* Группы вкладок (фича 014): одна или две, сколько показать — решает ядро
            по ширине окна. FR-006, FR-007 фичи 002: доска — вкладка того же
            окна, отдельного окна не открывается. При активной доске и одной
            группе колонки предпросмотра нет (research.md §9): доска занимает
            место пары «редактор + предпросмотр», при двух группах колонка
            остаётся с пустым состоянием, чтобы раскладка не прыгала. */}
        {layout !== null && registry !== null ? (
          <EditorGroups
            view={layout}
            tabs={tabs}
            client={state.client}
            registry={registry}
            boardControllers={boardControllers}
            shortcuts={shortcuts}
            editor={{
              boards,
              externalChange: lastChange,
              onOpenBoard: openBoard,
              onCreateBoard: () => void createBoard(),
              onEditorReady: editorReady,
              onCaretOffset: noteOutline.noticeCaret,
              onSaveAsTemplate: setSavingTemplate,
              fontSize: editorFontSize,
              shortcuts,
              paramsRequest,
              selectRequest,
              onLibraryStatus: (status) => setLibraryCustomized(status === "Customized"),
              attachments: noteAttachments,
              sources: noteSources,
              completion: noteCompletion,
            }}
            linksSummary={linksSummary ?? undefined}
            attachmentStatus={
              attachments.lastInserted === null
                ? undefined
                : t("attachments.status.inserted", { name: attachments.lastInserted })
            }
            solo={!showPreview}
          />
        ) : (
          <div className="ts-groups ts-groups--solo" aria-busy="true" />
        )}

        {showPreview && (
          <>
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

            {isBoardOpen ? (
              <section className="ts-pane ts-pane--preview" aria-label={t("preview.label")}>
                <div className="ts-placeholder">
                  <p role="status">{t("workspace.tabs.boardNoPreview")}</p>
                </div>
              </section>
            ) : (
              /* Компилятор от открытого vault приоритетнее переданного пропом:
                 он делит с ним очередь вызовов ядра, а проп остаётся для
                 оболочек, которые дают предпросмотр отдельно. */
              <PreviewPane
                client={state.preview ?? preview}
                notePath={openNotePath}
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
                // Фича 005: переход к месту замечания и пересборка при правке
                // связанного файла.
                onReveal={revealPlace}
                canReveal={canRevealPath}
                onDependencies={notePreviewDependencies}
                // Фича 019: показ слайдов следит за сборкой этой заметки.
                onBuild={noteBuild}
                // Фича 015: переход из оглавления доводит и предпросмотр.
                headingRequest={headingRequest}
              />
            )}
          </>
        )}
      </div>
      {phone && (
        <nav className="ts-phone-switch" aria-label={t("workspace.phone.switch")}>
          {(["tree", "editor", "preview"] as const).map((pane) => (
            <button
              key={pane}
              type="button"
              className="ts-phone-switch__btn"
              aria-pressed={shownPhonePane === pane}
              disabled={pane === "preview" && !showPreview}
              onClick={() => setPhonePane(pane)}
            >
              {t(PHONE_PANE_LABEL[pane])}
            </button>
          ))}
        </nav>
      )}
      </>
      )}

      {drawer && noteSheetOpen && view === "workspace" && (
        <NotePanelDrawer
          modes={noteModes}
          active={notePanel.mode}
          onSelect={notePanel.setMode}
          onClose={closeNoteDrawer}
          onEscape={escapeNoteDrawer}
        />
      )}

      {narrow && noteSheetOpen && view === "workspace" && (
        <NotePanel
          modes={noteModes}
          active={notePanel.mode}
          onSelect={notePanel.setMode}
          presentation="sheet"
          onClose={closeNoteSheet}
        />
      )}

      {openNotesSheet && layout !== null && layout.presentation === "List" && view === "workspace" && (
        <OpenNotesSheet
          view={layout}
          onActivate={(tab) => void tabs.activate(tab)}
          onClose={(tab) => void tabs.close(tab)}
          onCloseOthers={(tab) => void tabs.closeOthers(tab)}
          onDismiss={() => setOpenNotesSheet(false)}
        />
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

      {propertiesWorkspace.reindex !== null && (
        <div className="ts-reindex" role="status">
          <span>
            {t("properties.reindex.running", {
              done: formatNumber(propertiesWorkspace.reindex.done),
              total: formatNumber(propertiesWorkspace.reindex.total),
            })}
          </span>
          <button type="button" className="ts-btn ts-btn--quiet" onClick={propertiesWorkspace.cancelReindex}>
            {t("properties.reindex.cancel")}
          </button>
        </div>
      )}

      {searchOpen && (
        <SearchOverlay
          client={state.client}
          currentNote={activeNote}
          initialConditions={searchConditions}
          onInsertSelection={(conditions) => {
            setSearchOpen(false);
            setSearchConditions([]);
            setSelectRequest((current) => ({ id: (current?.id ?? 0) + 1, conditions }));
          }}
          onOpenNote={(path) => {
            const note = state.notes.find((entry) => entry.path === path);
            if (note) openEntry(note);
            setView("workspace");
          }}
          onClose={() => {
            setSearchOpen(false);
            setSearchConditions([]);
          }}
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
          onReveal={revealPlace}
        />
      )}

      {importer.dialogOpen && <ImportDialog control={importer} />}

      {captureSheet}

      {importer.run !== null && (
        <ImportProgress
          run={importer.run}
          onCancel={importer.cancel}
          onToggleMinimized={importer.toggleMinimized}
          onDismiss={importer.dismissRun}
          onOpenReport={(path) => {
            importer.dismissRun();
            void openImportReport(path);
          }}
          takeSnapshot={importTakeSnapshot}
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
          {...(openNotePath !== null
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
          suggestedName={active?.name ?? t("workspace.templateName")}
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
                { sync, onCloneVault: openClone },
              )}
            </div>
          </div>
        </div>
      )}

      {/* Синхронизация (фича 011). Панель не модальна: она сбоку, и работа с
          заметкой под ней продолжается (FR-019). */}
      {sync !== null && syncPanel !== null && (
        <SyncPanel
          sync={sync}
          focus={syncPanel.focus}
          onOpenFileHistory={openFileHistory}
          onClose={() => setSyncPanel(null)}
        />
      )}
      {sync !== null && rulesFile !== null && (
        <IgnoreEditor sync={sync} file={rulesFile} onClose={() => setRulesFile(null)} />
      )}
      {attachments.large !== null && (
        <LargeImageDialog request={attachments.large} onAnswer={attachments.answerLarge} />
      )}
      {imageSheetOpen && (
        <InsertImageSheet
          sources={imageSources}
          onPick={pickImages}
          onClose={() => setImageSheetOpen(false)}
        />
      )}
      {sync !== null && fileHistoryPath !== null && (
        <FileHistory
          sync={sync}
          path={fileHistoryPath}
          applyToEditor={applyHistoryToEditor}
          onClose={() => setFileHistoryPath(null)}
        />
      )}
      {sync !== null && sync.keeperConfirmation !== null && (
        <KeeperConfirmDialog request={sync.keeperConfirmation} />
      )}
      {cloneDialog}

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
 * «Назад», «Вперёд» и название открытой заметки в шапке телефона (макет «Вкладки»
 * 1c): полосы вкладок там нет, история остаётся.
 */
function PhoneNav({
  layout,
  onBack,
  onForward,
  backHint,
  forwardHint,
}: {
  layout: WorkspaceView;
  onBack: () => void;
  onForward: () => void;
  backHint: string;
  forwardHint: string;
}): React.JSX.Element {
  const group = layout.groups.find((entry) => entry.slot === layout.active_group) ?? layout.groups[0];
  const title = group?.tabs.find((tab) => tab.id === group.active)?.title ?? null;
  return (
    <span className="ts-phone-nav">
      <button
        type="button"
        className="ts-btn ts-btn--icon"
        disabled={group?.can_back !== true}
        title={backHint}
        aria-label={backHint}
        onClick={onBack}
      >
        <Icon name="chevron" size={12} className="ts-icon--flip" />
      </button>
      <button
        type="button"
        className="ts-btn ts-btn--icon"
        disabled={group?.can_forward !== true}
        title={forwardHint}
        aria-label={forwardHint}
        onClick={onForward}
      >
        <Icon name="chevron" size={12} />
      </button>
      {title !== null && <span className="ts-phone-nav__title">{title}</span>}
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

/**
 * Задевает ли событие vault один из путей. Массовое изменение путей не
 * называет, поэтому при непустом списке считается задевшим: пересобрать
 * лишний раз дешевле, чем показать устаревший вид.
 */
function touchesAny(event: VaultEvent, paths: ReadonlySet<string>): boolean {
  if (paths.size === 0) return false;
  switch (event.type) {
    case "NoteCreated":
    case "NoteUpdated":
    case "NoteDeleted":
      return paths.has(event.path);
    case "NoteRenamed":
      return paths.has(event.from_path) || paths.has(event.to_path);
    case "BulkChangeSettled":
      return true;
    default:
      return false;
  }
}
