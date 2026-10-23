import { invoke } from "./tauri-invoke.js";
import { listen } from "@tauri-apps/api/event";
import { createFlushRegistry, type VaultClient } from "./vault-client.js";
import type { PreviewClient } from "./preview-client.js";
import type { SyncClient } from "./sync/sync-client.js";
import { openTauriSync } from "./sync/sync-client-tauri.js";
// Из нейтрального модуля, а не из `vault-client-web.js`: тот импортирует
// `WebBackend` значением и втянул бы WASM в сборку оболочки.
import { basename } from "./paths.js";
import type {
  LastVault,
  VaultOpenResult,
  OpenVaultError,
} from "./vault-tree/useVault.js";
import type {
  BibliographyOffer,
  CitationPlan,
  CitationPlanError,
  CitationScanView,
  CitingNote,
  CreateSourceFileError,
  LibrarySummary,
  MissingKey,
  NoteCheck,
  RefreshStep,
  SourceHit,
  SourceHits,
  AttachmentBatchState,
  AttachmentBatchStep,
  AttachmentBatchView,
  AccessState,
  BacklinksPage,
  VaultGraph,
  CompletionList,
  DefinitionOutcome,
  LinkedIndex,
  ReferenceMark,
  CaptureOutcome,
  CaptureSettings,
  DailyDate,
  DailyNeighbours,
  LibraryStatus,
  OpenDaily,
  NoteProperties,
  ParsedConditions,
  PropertyKeyInfo,
  PropertyValue,
  ReindexOutcome,
  SelectBlock,
  SelectLibrary,
  TextEdit,
  Board,
  BoardEvent,
  BoardWriteProfile,
  BatchState,
  BatchStep,
  BatchView,
  CompiledDocument,
  ExportCandidate,
  ExportReport,
  DiffLine,
  EditConflict,
  Embed,
  EmbedPlan,
  EmbedRef,
  FileKind,
  FileStamp,
  ImportState,
  ImportSurvey,
  InsertionEdit,
  EditOutcome,
  ElementAtResult,
  ElementSpec,
  RenderOutcome,
  InstalledTemplate,
  InstallReport,
  LinkOutcome,
  LinkHealth,
  NoteHistory,
  DeletedNote,
  RecoverOutcome,
  RestoreOutcome,
  SnapshotReason,
  SnapshotUsage,
  TakeOutcome,
  SpellConfig,
  EmbedInsertion,
  AddOutcome,
  BookmarksView,
  LinkFixRequest,
  MentionScan,
  MentionStep,
  MentionsAvailability,
  MentionsStart,
  MoveImpact,
  MoveOutcome,
  MoveTarget,
  NameCheck,
  Note,
  Outline,
  OutlineView,
  PreviewState,
  ResolvedAnchor,
  ResolvedBinding,
  SavedWorkspace,
  SearchResults,
  ReplacePreview,
  ReplaceNoteOutcome,
  UndoOutcome,
  ShowKeymap,
  ShowState,
  SlideLabel,
  SlidePlace,
  SourceGroup,
  Template,
  TemplateEvent,
  TemplateLibraryView,
  Transition,
  VaultEvent,
  VaultOverrides,
  WorkspaceState,
  WorkspaceView,
} from "./wasm-core/typsidian_core.js";

// Реализация `VaultClient` поверх команд Tauri (001 T113, T114, T115) —
// третья наравне с web (`vault-client-web.ts`) и общая для обеих нативных
// оболочек, рабочего стола и Android.
//
// Каждый метод — один `invoke` соответствующей команды Tauri, за которой стоит
// `NativeBackend` (`packages/shell/src/lib.rs`). Правил здесь нет: их нельзя
// завести, не продублировав ядро (принцип I).
//
// Типы взяты из тех же сгенерированных из Rust определений, что и на web
// (принцип IV): импорт типовой, WASM в сборку оболочки не попадает — ядро здесь
// нативное.
//
// Чего в этом модуле НЕТ — жеста выбора папки: он у платформ разный
// (`apps/desktop` — нативный диалог ОС, `apps/mobile` — свой обзор папок), и
// именно им отличаются оболочки. Общее начинается с пути: см. `openVaultAt`.

/** Имя события, которым оболочка отдаёт окну пачку `VaultEvent`. */
const VAULT_EVENTS = "vault://events";

/**
 * Открыть vault по уже выбранному пути.
 *
 * Здесь всё, что одинаково у рабочего стола и мобильного: открытие в ядре,
 * классификация отказа и два разовых запроса к оболочке. Различается только то,
 * откуда взялся `path`, — и это остаётся в оболочках.
 *
 * Классификация отказов та же, что у web (`OpenVaultError`). Причины, которых
 * на native не бывает (`NotSupported` — нет File System Access API), здесь
 * просто не возникают, а `TooManyNotes` приходит из того же ядра.
 */
export async function openVaultAt(path: string): Promise<VaultOpenResult> {
  try {
    await invoke<void>("open_vault", { path });
  } catch (cause) {
    return { ok: false, error: classifyNativeOpenFailure(cause) };
  }
  // Профиль спрашивается у оболочки один раз, здесь: он выбран её сборкой и
  // за время работы окна не меняется, а `VaultClient.boardWriteProfile`
  // синхронное — ждать `invoke` на каждом рендере доски незачем (FR-035).
  const profile = await invoke<BoardWriteProfile>("board_write_profile");
  // Корень спрашивается один раз: пока окно живёт, открытый vault не меняется,
  // а `VaultClient.rootName` синхронное.
  const root = await invoke<string>("root_path");
  // FR-007: открытый vault становится последним. Здесь, а не в оболочках:
  // запоминается то же самое и там, и там, и разъехаться этому нельзя.
  //
  // Отказ хранилища настроек открытие не отменяет: vault уже открыт и работает,
  // а не запомнилась только точка входа следующего запуска.
  //
  // Идентификатор записи нужен синхронизации (фича 011): под ним лежат
  // значения устройства и секреты этого vault. Не запомнился vault — нет и
  // сессии синхронизации; сам vault при этом работает (FR-038).
  let sync: SyncClient | undefined;
  let settingsNotice: VaultEvent | undefined;
  try {
    const remembered = await invoke<{ id: string; notice?: VaultEvent }>("remember_vault", {
      path,
    });
    settingsNotice = remembered.notice;
    sync = await openTauriSync(remembered.id);
  } catch {
    // Молча: сказать об этом пользователю нечего — действия от него не требуется.
  }
  return {
    ok: true,
    client: tauriVaultClient(profile, basename(root)),
    ...(sync === undefined ? {} : { sync }),
    ...(settingsNotice === undefined ? {} : { settingsNotice }),
    // Предпросмотр приходит вместе с клиентом, как и на web: движок живёт в
    // оболочке всё время работы окна, и отдельного создания ему не нужно.
    preview: tauriPreviewClient(),
  };
}

/**
 * Открыть известный vault по его записи — жест переключения из панели настроек
 * (US2 фичи 004, AS3). Тот же `openWebVaultById` на web.
 */
export async function openTauriVaultById(id: string): Promise<VaultOpenResult> {
  let path: string | null;
  try {
    path = await invoke<string | null>("resolve_known_vault", { id });
  } catch (cause) {
    return { ok: false, error: { kind: "OpenFailed", detail: String(cause) } };
  }
  // Папки больше нет, а розыск запрещён (FR-023) — та же причина, что на web.
  if (path === null) return { ok: false, error: { kind: "FolderNotFound" } };
  return openVaultAt(path);
}

/**
 * Последний открытый vault по данным хранилища настроек устройства (FR-007).
 *
 * Общий для рабочего стола и Android: хранилище там одно и то же —
 * `settings.json` ядра в каталоге конфигурации платформы, а команды к нему
 * живут в `packages/shell`.
 *
 * `null` — предлагать нечего: ни одного vault ещё не открывали либо папка
 * больше не разрешается по записанному пути. Перенесённая папка **не
 * разыскивается** (FR-023 фичи 004) — это решение ядра, а не молчание этого
 * модуля.
 */
export async function lastTauriVault(): Promise<LastVault | null> {
  let record: LastVaultRecord | null;
  try {
    record = await invoke<LastVaultRecord | null>("last_vault");
  } catch {
    // Хранилище настроек недоступно — предложить продолжить не выйдет, но
    // выбор папки работает по-прежнему.
    return null;
  }
  if (record === null) return null;
  return {
    // Подпись пользователя важнее пути: он сам назвал этот vault так.
    label: record.label ?? record.location,
    // Спрашивать нечего: путь уже разрешился в существующую папку, а прав на
    // файловую систему у нативной сборки не отзывают между запусками. Поэтому
    // окно открывает его само, без единого нажатия.
    readyToOpen: true,
    open: () => openVaultAt(record.path),
  };
}

/** Ответ команды `last_vault` — `LastVaultView` оболочки. */
interface LastVaultRecord {
  label: string | null;
  location: string;
  path: string;
}

/**
 * Предпросмотр документа поверх команд Tauri (фича 005, T042).
 *
 * Состояния у клиента нет: шрифты и кэш шаблонов держит оболочка
 * (`typsidian_shell::Preview`), а сюда приходят только вызовы. На web их
 * приходится проводить через `wasm-call-queue`, потому что там движок и vault
 * делят одно заимствование на границе wasm-bindgen; здесь такой границы нет, и
 * очередь была бы обрядом без причины.
 */
function tauriPreviewClient(): PreviewClient {
  return {
    dependencyMark: (paths) => invoke<string>("preview_dependency_mark", { paths }),
    completionLinkedIndex: (notePath, dependencies) =>
      invoke<LinkedIndex>("completion_linked_index", { notePath, dependencies }),
    compile: (notePath, text, revision, anchor) =>
      invoke<CompiledDocument>("preview_compile", { notePath, text, revision, anchor }),
    // Фича 007: то же, но недостающие внешние шаблоны доустанавливаются. Это
    // отдельная команда, а не флаг: путь может писать в vault и ходить в сеть.
    compileWithTemplates: (notePath, text, revision, overrides, anchor) =>
      invoke<CompiledDocument>("preview_compile_with_templates", {
        notePath,
        text,
        revision,
        overrides,
        anchor,
      }),
    // FR-010: явная установка — настройка автозагрузки здесь не спрашивается.
    installTemplate: (spec, overrides) =>
      invoke<InstallReport>("preview_install_template", { spec, overrides }),
    installedTemplates: () =>
      invoke<InstalledTemplate[]>("preview_installed_templates", {}),
    // Фича 009: экспорт идёт командой в пуле tokio, вне потока окна.
    exportPdf: (notePath, text, overrides) =>
      invoke<ExportReport>("export_pdf", { notePath, text, overrides }),
    exportCandidates: (overrides) =>
      invoke<ExportCandidate[]>("export_candidates", { overrides }),
    // Фича 015: без состояния оболочки — прогоны страниц приходят из окна.
    locateHeading: (runs, outline, key) =>
      invoke<ResolvedAnchor | null>("preview_locate_heading", { runs, outline, key }),
    exportBatch: {
      start: (paths, stopOnError, overrides) =>
        invoke<BatchState>("export_batch_start", { paths, stopOnError, overrides }),
      next: (state) => invoke<BatchStep>("export_batch_next", { state }),
      record: (state, report, elapsedMs) =>
        invoke<BatchState>("export_batch_record", {
          state,
          report,
          elapsedMs: Math.round(elapsedMs),
        }),
      skip: (state, notePath) =>
        invoke<BatchState>("export_batch_skip", { state, notePath }),
      cancel: (state) => invoke<BatchState>("export_batch_cancel", { state }),
      view: (state) => invoke<BatchView>("export_batch_view", { state }),
    },
  };
}

/**
 * Отказ открытия от ядра — теми же вариантами, что на web.
 *
 * `OpenError` пересекает границу как размеченное объединение, поэтому здесь
 * сужение по `kind`, а не разбор строки.
 */
function classifyNativeOpenFailure(cause: unknown): OpenVaultError {
  switch (kindOf(cause)) {
    case "TooManyNotes":
      return { kind: "TooManyNotes" };
    case "AccessDenied":
      return { kind: "PermissionDenied" };
    case "NotFound":
      return { kind: "FolderNotFound" };
    case "NotADirectory":
      return { kind: "NotADirectory" };
    default:
      return { kind: "OpenFailed", detail: String(cause) };
  }
}

function kindOf(cause: unknown): string | null {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const { kind } = cause as { kind: unknown };
    return typeof kind === "string" ? kind : null;
  }
  return null;
}

export function tauriVaultClient(
  boardWriteProfile: BoardWriteProfile,
  rootName: string,
): VaultClient {
  // Снимок «есть ли что терять» для `beforeunload`, где ждать промис нельзя.
  //
  // Обновляется двумя путями: пессимистично — сразу при объявлении правок, и
  // точно — после каждой операции, которая могла их снять. Ошибаться этому
  // снимку позволено только в сторону «есть несохранённое» (см. `VaultClient`),
  // поэтому `beginEdit` ставит флаг синхронно, не дожидаясь ответа ядра.
  let unresolved = false;

  const refreshUnresolved = async (): Promise<string[]> => {
    const paths = await invoke<string[]>("unresolved_edit_paths");
    unresolved = paths.length > 0;
    return paths;
  };

  return {
    list: () => invoke<Note[]>("list"),
    read: (path) => invoke<string>("read", { path }),
    write: async (path, content, expected) => {
      await invoke<void>("write", { path, content, expected: expected ?? null });
      await refreshUnresolved();
    },
    create: (path) => invoke<void>("create", { path }),
    rename: (from, to) => invoke<void>("rename", { from, to }),
    delete: (path) => invoke<void>("delete", { path }),
    moveTargets: (from, query) => invoke<MoveTarget[]>("move_targets", { from, query }),
    moveImpact: (request) => invoke<MoveImpact>("move_impact", { request }),
    // На рабочем столе перенос — один атомарный системный вызов: ни хода, ни
    // отмены у него нет (023 research.md §5).
    moveEntry: (request) => invoke<MoveOutcome>("move_entry", { request }),
    backlinks: (path) => invoke<Note[]>("backlinks", { path }),
    graph: (request) => invoke<VaultGraph>("vault_graph", { request }),
    completions: (request) => invoke<CompletionList>("completions", { request }),
    definitionAt: (request) => invoke<DefinitionOutcome>("definition_at", { request }),
    referenceMarks: (request) => invoke<ReferenceMark[]>("reference_marks", { request }),
    brokenLinks: (path) => invoke<string[]>("broken_links", { path }),
    accessState: () => invoke<AccessState>("access_state"),
    search: (query) => invoke<SearchResults>("search", { query }),
    replacePreview: (query, openTexts) =>
      invoke<ReplacePreview>("replace_preview", { query, openTexts }),
    replaceInNote: (request, editorText) =>
      invoke<ReplaceNoteOutcome>("replace_in_note", { request, editorText }),
    replaceUndo: (record, editorText) =>
      invoke<UndoOutcome>("replace_undo", { record, editorText }),
    fileKind: (path) => invoke<FileKind>("file_kind", { path }),

    beginEdit: async (path, buffer) => {
      unresolved = true;
      await invoke<void>("begin_edit", { path, buffer });
    },
    discardEdit: async (path) => {
      await invoke<void>("discard_edit", { path });
      await refreshUnresolved();
    },
    pendingConflict: async (path) => {
      const conflict = await invoke<EditConflict | null>("pending_conflict", { path });
      return conflict ?? undefined;
    },
    conflictDiff: (path) => invoke<DiffLine[]>("conflict_diff", { path }),
    resolveConflictKeepLocal: async (path) => {
      await invoke<void>("resolve_conflict_keep_local", { path });
      await refreshUnresolved();
    },
    resolveConflictAcceptExternal: async (path) => {
      const external = await invoke<string | null>("resolve_conflict_accept_external", {
        path,
      });
      await refreshUnresolved();
      return external ?? undefined;
    },
    unresolvedEditPaths: refreshUnresolved,
    ...createFlushRegistry(refreshUnresolved),
    hasUnresolvedEdits: () => unresolved,

    subscribe: (onEvents) => {
      // Push, а не опрос: ядро сигналит появление событий, оболочка пересылает
      // их в окно одним `emit` (T114). Подписка асинхронна, поэтому отписка
      // дожидается её же промиса — иначе гонка при быстром размонтировании
      // оставила бы слушателя навсегда.
      let stop: (() => void) | null = null;
      let cancelled = false;
      void listen<VaultEvent[]>(VAULT_EVENTS, (event) => {
        onEvents(event.payload);
        // Правки могли быть сняты извне (файл удалён, конфликт разрешён в
        // другом окне) — снимок обязан это догнать.
        void refreshUnresolved();
      }).then((unlisten) => {
        if (cancelled) unlisten();
        else stop = unlisten;
      });
      return () => {
        cancelled = true;
        stop?.();
      };
    },

    boardWriteProfile,
    rootName,

    createBoard: (path) => invoke<Board>("create_board", { path }),
    readBoard: (path) => invoke<string>("read_board", { path }),
    writeBoard: (path, content, expected) =>
      invoke<void>("write_board", { path, content, expected: expected ?? null }),
    writeSnapshot: (snapshotPath, svg, boardPath, boardContent) =>
      invoke<void>("write_snapshot", {
        snapshotPath,
        svg,
        boardPath,
        boardContent,
      }),
    embedsOf: (notePath, noteText) =>
      invoke<Embed[]>("embeds_of", { notePath, noteText }),
    embedsInText: (notePath, noteText) =>
      invoke<EmbedRef[]>("embeds_in_text", { notePath, noteText }),
    embedsChanged: async (notePath, noteText, changed) => {
      const event = await invoke<BoardEvent | null>("embeds_changed", {
        notePath,
        noteText,
        changed,
      });
      return event ?? undefined;
    },
    planEmbed: (notePath, noteText, boardPath, overrides) =>
      invoke<EmbedPlan>("plan_embed", {
        notePath,
        noteText,
        boardPath,
        overrides,
      }),
    embedMarkup: (plan) => invoke<string>("embed_markup", { plan }),

    // ── Шаблоны и сниппеты (фича 003) ─────────────────────────────────
    templateLibrary: () => invoke<TemplateLibraryView>("template_library"),
    readTemplate: (path) => invoke<string>("read_template", { path }),
    templatePreview: (template) =>
      invoke<PreviewState>("template_preview", { template }),
    checkTemplateName: (name) => invoke<NameCheck>("check_template_name", { name }),
    saveAsTemplate: (name, content, confirmOverwrite) =>
      invoke<Template>("save_as_template", {
        name,
        content,
        confirmOverwrite,
      }),
    planInsertion: (textLength, point, content) =>
      invoke<InsertionEdit>("plan_insertion", { textLength, point, content }),
    // Фича 008: те же функции ядра командами Tauri.
    elementSchema: async (id) =>
      (await invoke<ElementSpec | null>("element_schema", { id })) ?? undefined,
    elementSchemaFor: async (template) =>
      (await invoke<ElementSpec | null>("element_schema_for", { template })) ?? undefined,
    elementAt: (text, offset) =>
      invoke<ElementAtResult>("element_at", { text, offset }),
    // Фича 015: команды без состояния оболочки, `Outline` держит окно.
    outline: (text) => invoke<Outline>("structure_outline", { text }),
    outlineView: (outline, request) =>
      invoke<OutlineView>("structure_outline_view", { outline, request }),
    outlineActive: (outline, request) =>
      invoke<string | null>("structure_outline_active", { outline, request }),
    outlineCarry: (prev, next, collapsed) =>
      invoke<string[]>("structure_outline_carry", { prev, next, collapsed }),
    // Фича 024: сессия живёт в состоянии оболочки под номером. Ответ IPC —
    // массив чисел, интерфейсу нужен тот же `Uint32Array`, что на web.
    openHighlight: (text) => {
      const id = invoke<number>("syntax_session_open", { text });
      return {
        update: async (edits, length, windows) => {
          const packed = await invoke<number[] | null>("syntax_session_update", {
            id: await id,
            edits,
            length,
            windows,
          });
          return packed === null ? undefined : Uint32Array.from(packed);
        },
        spelling: async (windows) => {
          const pairs = await invoke<number[] | null>("syntax_session_spelling", {
            id: await id,
            windows,
          });
          return pairs === null ? undefined : Uint32Array.from(pairs);
        },
        close: () => {
          void id.then((session) => invoke("syntax_session_close", { id: session }));
        },
      };
    },
    renderElement: (request) => invoke<RenderOutcome>("render_element", { request }),
    planElementEdit: (request) =>
      invoke<EditOutcome>("plan_element_edit", { request }),
    templateEvent: async (event) => {
      const effect = await invoke<TemplateEvent | null>("template_event", {
        event,
      });
      return effect ?? undefined;
    },

    // ── Настройки уровня vault и разделы (фича 006) ───────────────────
    vaultSettings: () => invoke<VaultOverrides>("vault_settings"),
    setVaultSettings: (overrides) =>
      invoke<void>("set_vault_settings", { overrides }),
    markSection: (folder) => invoke<VaultOverrides>("mark_section", { folder }),
    unmarkSection: (folder) =>
      invoke<VaultOverrides>("unmark_section", { folder }),

    // ── Сочетания клавиш ──────────────────────────────────────────────
    keybindings: (overrides, mac) =>
      invoke<ResolvedBinding[]>("keybindings", { overrides, mac }),
    setKeybinding: (command, shortcut) =>
      invoke<VaultOverrides>("set_keybinding", { command, shortcut }),
    resetKeybinding: (command) =>
      invoke<VaultOverrides>("reset_keybinding", { command }),

    // ── Вкладки и разделение редактора (фича 014) ─────────────────────
    // Состояние вкладок в команде называется `workspace`, а не `state`: `state`
    // в Tauri занят служебным параметром.
    workspaceEmpty: () => invoke<WorkspaceState>("workspace_empty"),
    workspaceApply: (workspace, action, widthPx) =>
      invoke<Transition>("workspace_apply", { workspace, action, widthPx }),
    workspaceView: (workspace, widthPx) =>
      invoke<WorkspaceView>("workspace_view", { workspace, widthPx }),
    workspaceSnapshot: (workspace) =>
      invoke<SavedWorkspace>("workspace_snapshot", { workspace }),
    workspaceRestore: (saved) => invoke<WorkspaceState>("workspace_restore", { saved }),

    // ── Связи заметки (фича 013) ──────────────────────────────────────
    // Состояние прохода в команде называется `scan`, а не `state`: `state` в
    // Tauri занят служебным параметром.
    links: {
      backlinksPage: (target, openTexts, offset, limit) =>
        invoke<BacklinksPage>("links_backlinks_page", { target, openTexts, offset, limit }),
      mentionsAvailability: (target) =>
        invoke<MentionsAvailability>("links_mentions_availability", { target }),
      mentionsStart: (target) => invoke<MentionsStart>("links_mentions_start", { target }),
      mentionsStep: (scan, openTexts) =>
        invoke<MentionStep>("links_mentions_step", { scan, openTexts }),
      mentionsCancel: (scan) => invoke<MentionScan>("links_mentions_cancel", { scan }),
      mentionsIn: (target, paths, openTexts) =>
        invoke<SourceGroup[]>("links_mentions_in", { target, paths, openTexts }),
      linkMention: (request, editorText) =>
        invoke<LinkOutcome>("links_link_mention", { request, editorText }),
      health: (openTexts) => invoke<LinkHealth>("links_health", { openTexts }),
      fix: (request, editorText) =>
        invoke<ReplaceNoteOutcome>("links_fix", { request, editorText }),
    },

    // ── Закладки (фича 032) ─────────────────────────────────────────
    bookmarks: {
      view: () => invoke<BookmarksView>("bookmarks_view"),
      add: (target, name) => invoke<AddOutcome>("bookmark_add", { target, name }),
      remove: (id) => invoke<BookmarksView>("bookmark_remove", { id }),
      rename: (id, name) => invoke<BookmarksView>("bookmark_rename", { id, name }),
      move: (id, place) => invoke<BookmarksView>("bookmark_move", { id, place }),
      groupCreate: (name) => invoke<BookmarksView>("bookmark_group_create", { name }),
      groupRename: (id, name) => invoke<BookmarksView>("bookmark_group_rename", { id, name }),
      groupToggle: (id) => invoke<BookmarksView>("bookmark_group_toggle", { id }),
      groupDelete: (id) => invoke<BookmarksView>("bookmark_group_delete", { id }),
    },

    // ── Встраивание заметок (фича 031) ──────────────────────────────
    embeds: {
      insert: (text, offset) => invoke<EmbedInsertion>("embed_insert", { text, offset }),
    },

    // ── Проверка орфографии (фича 030) ──────────────────────────────
    // Словари вшиты в нативное ядро; проверяющий — состояние оболочки.
    spelling: {
      configure: () => invoke<SpellConfig>("spell_configure"),
      addWord: (word) => invoke<SpellConfig>("spell_add_word", { word }),
      removeWord: (word) => invoke<SpellConfig>("spell_remove_word", { word }),
      suggest: (word) => invoke<string[]>("spell_suggest", { word }),
    },

    // ── Снимки заметок (фича 029) ─────────────────────────────────────
    snapshots: {
      take: (path, text, reason) => invoke<TakeOutcome>("snapshot_take", { path, text, reason }),
      history: (path) => invoke<NoteHistory>("snapshot_history", { path }),
      diff: (id, against) => invoke<DiffLine[]>("snapshot_diff", { id, against }),
      text: (id) => invoke<string | null>("snapshot_text", { id }),
      restore: (path, id, editorText) =>
        invoke<RestoreOutcome>("snapshot_restore", { path, id, editorText }),
      deleted: () => invoke<DeletedNote[]>("snapshot_deleted_notes"),
      recover: (path, asPath) => invoke<RecoverOutcome>("snapshot_recover_note", { path, asPath }),
      usage: () => invoke<SnapshotUsage>("snapshot_usage"),
      clear: () => invoke<number>("snapshots_clear"),
    },

    // ── Показ слайдов (фича 019) ──────────────────────────────────────
    // Чистые функции ядра. Отсутствующая каретка уходит как `null`: поле
    // `Option` в команде Tauri.
    slides: {
      start: (slides, caret, nowMs) =>
        invoke<ShowState>("slides_start", { slides, caret: caret ?? null, nowMs }),
      navigate: (state, slides, action) =>
        invoke<ShowState>("slides_navigate", { state, slides, action }),
      carry: (state, slides) => invoke<ShowState>("slides_carry", { state, slides }),
      caret: async (slides, page) => {
        const place = await invoke<SlidePlace | null>("slides_caret", { slides, page });
        return place ?? undefined;
      },
      label: (slides, page) => invoke<SlideLabel>("slides_label", { slides, page }),
      keymap: () => invoke<ShowKeymap>("slides_keymap"),
    },

    // ── Свойства заметок и выборки (фича 020) ─────────────────────────
    // Команды Tauri 1:1 с функциями ядра. Нет значения ячейки приходит как
    // `null` — клиенту нужен тот же `undefined`, что на web.
    properties: {
      read: (text) => invoke<NoteProperties>("properties_read", { text }),
      edit: (text, change) => invoke<EditOutcome>("properties_edit", { text, change }),
      selectBlocks: (text) => invoke<SelectBlock[]>("select_blocks", { text }),
      selectImportEdits: (text) => invoke<TextEdit[]>("select_import_edits", { text }),
      selectLibrary: () => invoke<SelectLibrary>("select_library"),
      parseConditions: (text, knownKeys) =>
        invoke<ParsedConditions>("search_parse_conditions", { text, knownKeys }),
      select: async (selection) => {
        const result = await invoke<{
          rows: { path: string; cells: (PropertyValue | null)[] }[];
          columns: string[];
          truncated: boolean;
          total: number;
        }>("vault_select", { selection });
        return {
          ...result,
          rows: result.rows.map((row) => ({
            path: row.path,
            cells: row.cells.map((cell) => cell ?? undefined),
          })),
        };
      },
      keys: () => invoke<PropertyKeyInfo[]>("vault_property_keys"),
      current: () => invoke<boolean>("vault_properties_current"),
      // Ход приходит событием, отмена — отдельной командой: замок vault между
      // шагами оболочка отпускает сама.
      reindex: async (control) => {
        const unlisten = await listen<{ done: number; total: number }>(
          "properties://reindex-progress",
          (event) => control?.onProgress?.(event.payload.done, event.payload.total),
        );
        const onAbort = (): void => {
          void invoke<void>("vault_cancel_reindex_properties").catch(() => undefined);
        };
        if (control?.signal?.aborted === true) onAbort();
        control?.signal?.addEventListener("abort", onAbort, { once: true });
        try {
          return await invoke<ReindexOutcome>("vault_reindex_properties");
        } finally {
          control?.signal?.removeEventListener("abort", onAbort);
          unlisten();
        }
      },
      ensureSelectLibrary: () => invoke<LibraryStatus>("vault_ensure_select_library"),
      selectDigest: async (text) =>
        (await invoke<string | null>("vault_select_digest", { text })) ?? undefined,
    },

    // ── Быстрый захват и ежедневные заметки (фича 022) ────────────────
    // Команды Tauri 1:1 с функциями ядра, аргументы camelCase. Нет значения
    // приходит как `null`; байты картинки идут числами JSON.
    capture: {
      settings: (overrides) => invoke<CaptureSettings>("capture_settings", { overrides }),
      entry: (settings, capture, imagePath) =>
        invoke<string>("capture_entry", { settings, capture, imagePath: imagePath ?? null }),
      targetPath: async (settings, capture) =>
        (await invoke<string | null>("capture_target_path", { settings, capture })) ?? undefined,
      dailyPath: async (settings, date) =>
        (await invoke<string | null>("daily_path", { settings, date })) ?? undefined,
      dailyDateOf: async (settings, path) =>
        (await invoke<DailyDate | null>("daily_date_of", { settings, path })) ?? undefined,
      write: (capture, openBuffer, image) =>
        invoke<CaptureOutcome>("capture_write", {
          capture,
          openBuffer: openBuffer ?? null,
          image: image === undefined ? null : Array.from(image),
        }),
      openDaily: (date) => invoke<OpenDaily>("open_daily", { date }),
      dailyNeighbours: (date) => invoke<DailyNeighbours>("daily_neighbours", { date }),
    },

    // ── Источники (фича 017) ──────────────────────────────────────────
    sources: {
      refresh: (registered) => invoke<RefreshStep>("sources_refresh", { registered }),
      summary: () => invoke<LibrarySummary>("sources_summary"),
      search: (query, limit, uncitedOnly) =>
        invoke<SourceHits>("sources_search", { query, limit, uncitedOnly }),
      forCompletion: (prefix, limit) =>
        invoke<SourceHit[]>("sources_for_completion", { prefix, limit }),
      scanStart: () => invoke<CitationScanView>("sources_scan_start"),
      scanStep: (maxNotes) => invoke<CitationScanView>("sources_scan_step", { maxNotes }),
      scanCancel: () => invoke<CitationScanView>("sources_scan_cancel"),
      noteChanged: (path) => invoke<void>("sources_note_changed", { path }),
      noteRemoved: (path) => invoke<void>("sources_note_removed", { path }),
      noteRenamed: (from, to) => invoke<void>("sources_note_renamed", { from, to }),
      citing: (key) => invoke<CitingNote[]>("sources_citing", { key }),
      missingKeys: () => invoke<MissingKey[]>("sources_missing_keys"),
      checkNote: (text, knownLabels) =>
        invoke<NoteCheck>("sources_check_note", { text, knownLabels }),
      planCitation: async (text, caret, file, key) => {
        try {
          const plan = await invoke<CitationPlan>("sources_plan_citation", {
            text,
            caret,
            file,
            key,
          });
          return { ok: true, plan };
        } catch (error) {
          return { ok: false, error: error as CitationPlanError };
        }
      },
      bibliographyOffer: (text, notePath, file) =>
        invoke<BibliographyOffer>("sources_bibliography_offer", { text, notePath, file }),
      createFile: async () => {
        try {
          return { ok: true, path: await invoke<string>("sources_create_file") };
        } catch (error) {
          return { ok: false, error: error as CreateSourceFileError };
        }
      },
    },

    // ── Вложения (фича 016) ───────────────────────────────────────────
    attachments: {
      start: (overrides, notePath, items, moment) =>
        invoke<AttachmentBatchState>("attachments_batch_start", {
          overrides,
          notePath,
          items,
          moment,
        }),
      next: (state) => invoke<AttachmentBatchStep>("attachments_batch_next", { state }),
      confirm: (state, index, accept) =>
        invoke<AttachmentBatchState>("attachments_batch_confirm", { state, index, accept }),
      // Байты — сырым телом (research.md §11): длина JSON в 4 байтах
      // little-endian, JSON `{state, index}`, затем файл. Массив чисел JSON
      // вырос бы в 3–4 раза.
      store: (state, index, bytes) => {
        const head = new TextEncoder().encode(JSON.stringify({ state, index }));
        const body = new Uint8Array(4 + head.length + bytes.length);
        new DataView(body.buffer).setUint32(0, head.length, true);
        body.set(head, 4);
        body.set(bytes, 4 + head.length);
        return invoke<AttachmentBatchState>("attachments_batch_store", body);
      },
      sourceFailed: (state, index) =>
        invoke<AttachmentBatchState>("attachments_batch_source_failed", { state, index }),
      cancel: (state) => invoke<AttachmentBatchState>("attachments_batch_cancel", { state }),
      view: (state) => invoke<AttachmentBatchView>("attachments_batch_view", { state }),
      planMarkup: async (textLength, point, before, after, paths, block) =>
        (await invoke<InsertionEdit | null>("attachments_plan_markup", {
          textLength,
          point,
          before,
          after,
          paths,
          block,
        })) ?? undefined,
      validateFolder: (raw) => invoke<string>("attachments_validate_folder", { raw }),
    },

    // ── Импорт из Obsidian (фича 021) ─────────────────────────────────
    // Команды Tauri 1:1 с функциями ядра; источник держит оболочка, путь к нему
    // передаётся один раз. Отказ — `OpenError` либо `ImportError`.
    import: {
      open: async (target) => {
        if (target.kind !== "path") throw { kind: "NoSource" };
        await invoke<void>("import_open_source", { path: target.path });
      },
      close: () => invoke<void>("import_close_source"),
      survey: (job) => invoke<ImportSurvey>("import_survey", { job }),
      begin: (job) => invoke<ImportState>("import_begin", { job }),
      step: (state) => invoke<ImportState>("import_step", { state }),
      finish: (state) => invoke<string>("import_finish", { state }),
    },
  };
}

/**
 * `FileStamp` в сигнатурах выше приходит из тех же типов ядра; отдельного
 * объявления здесь нет намеренно — тип используется как есть.
 */
export type { FileStamp };
