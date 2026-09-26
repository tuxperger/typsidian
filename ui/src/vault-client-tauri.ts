import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { VaultClient } from "./vault-client.js";
import type { PreviewClient } from "./preview-client.js";
// Из нейтрального модуля, а не из `vault-client-web.js`: тот импортирует
// `WebBackend` значением и втянул бы WASM в сборку оболочки.
import { basename } from "./paths.js";
import type {
  LastVault,
  VaultOpenResult,
  OpenVaultError,
} from "./vault-tree/useVault.js";
import type {
  AccessState,
  Board,
  BoardEvent,
  BoardWriteProfile,
  CompiledDocument,
  DiffLine,
  EditConflict,
  Embed,
  EmbedPlan,
  EmbedRef,
  FileKind,
  FileStamp,
  InsertionEdit,
  EditOutcome,
  ElementAtResult,
  ElementSpec,
  RenderOutcome,
  InstalledTemplate,
  InstallReport,
  NameCheck,
  Note,
  PreviewState,
  ResolvedBinding,
  SearchResults,
  Template,
  TemplateEvent,
  TemplateLibraryView,
  VaultEvent,
  VaultOverrides,
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
  try {
    await invoke<void>("remember_vault", { path });
  } catch {
    // Молча: сказать об этом пользователю нечего — действия от него не требуется.
  }
  return {
    ok: true,
    client: tauriVaultClient(profile, basename(root)),
    // Предпросмотр приходит вместе с клиентом, как и на web: движок живёт в
    // оболочке всё время работы окна, и отдельного создания ему не нужно.
    preview: tauriPreviewClient(),
  };
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
    compile: (notePath, text, revision) =>
      invoke<CompiledDocument>("preview_compile", { notePath, text, revision }),
    // Фича 007: то же, но недостающие внешние шаблоны доустанавливаются. Это
    // отдельная команда, а не флаг: путь может писать в vault и ходить в сеть.
    compileWithTemplates: (notePath, text, revision, overrides) =>
      invoke<CompiledDocument>("preview_compile_with_templates", {
        notePath,
        text,
        revision,
        overrides,
      }),
    // FR-010: явная установка — настройка автозагрузки здесь не спрашивается.
    installTemplate: (spec, overrides) =>
      invoke<InstallReport>("preview_install_template", { spec, overrides }),
    installedTemplates: () =>
      invoke<InstalledTemplate[]>("preview_installed_templates", {}),
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
      return { kind: "OpenFailed", message: "NotFound" };
    case "NotADirectory":
      return { kind: "OpenFailed", message: "NotADirectory" };
    default:
      return { kind: "OpenFailed", message: String(cause) };
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
    backlinks: (path) => invoke<Note[]>("backlinks", { path }),
    brokenLinks: (path) => invoke<string[]>("broken_links", { path }),
    accessState: () => invoke<AccessState>("access_state"),
    search: (query) => invoke<SearchResults>("search", { query }),
    fileKind: (path) => invoke<FileKind>("file_kind", { path }),

    beginEdit: async (path, buffer) => {
      unresolved = true;
      await invoke<void>("begin_edit", { path, buffer });
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
    elementAt: (text, offset) =>
      invoke<ElementAtResult>("element_at", { text, offset }),
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
  };
}

/**
 * `FileStamp` в сигнатурах выше приходит из тех же типов ядра; отдельного
 * объявления здесь нет намеренно — тип используется как есть.
 */
export type { FileStamp };
