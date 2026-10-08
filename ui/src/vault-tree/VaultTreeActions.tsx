import { createContext, useCallback, useContext, useMemo, useState } from "react";
import type {
  CreateError,
  DeleteError,
  Note,
  RenameError,
  Template,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { currentI18n, useI18n, type Translate } from "../i18n/index.js";
import { fillFromTemplate } from "../templates/NewNoteFromTemplate.js";
import { buildTree, type TreeFolder, type TreeNode } from "./buildTree.js";
import type { VaultOperationError } from "./ErrorBanner.js";
import { isBoard } from "./fileKind.js";
import { Icon } from "../icons/Icons.js";
import { copyText, useContextMenu, type ContextMenuItem } from "../menu/ContextMenu.js";

// Принцип I конституции: этот файл не переизобретает vault-логику —
// create/rename/delete здесь всегда прямые вызовы `VaultClient`
// (contracts/vault-backend-trait.md). Единственное, что добавлено сверх
// ядра — UX вокруг них: контекстное меню, инлайн-режимы ввода имени,
// подтверждение перед удалением (T042).

function folderOf(path: string): string {
  const idx = path.lastIndexOf("/");
  return idx === -1 ? "" : path.slice(0, idx);
}

function joinPath(folder: string, name: string): string {
  return folder === "" ? name : `${folder}/${name}`;
}

// Ошибки пересекают границу WASM/JS как размеченные объединения (contracts/
// frontend-events.md), но сигнатуры `VaultClient.create/rename/delete` в
// сгенерированном `.d.ts` объявлены как `Promise<void>` — тип отклонённого
// промиса на стороне TS не виден. Эти guard'ы восстанавливают точный тип
// без `any`/`as`: каждый `if` сравнивает с ОДНИМ литералом, поэтому TS
// сужает `kind` до этого литерала естественным образом (control-flow
// narrowing по равенству), а не через приведение типа.
function asCreateError(cause: unknown): CreateError | null {
  if (typeof cause !== "object" || cause === null || !("kind" in cause)) {
    return null;
  }
  const { kind } = cause;
  if (kind === "NameConflict") return { kind };
  if (kind === "InvalidName") return { kind };
  if (kind === "Io") return { kind };
  return null;
}

function asRenameError(cause: unknown): RenameError | null {
  if (typeof cause !== "object" || cause === null || !("kind" in cause)) {
    return null;
  }
  const { kind } = cause;
  if (kind === "NameConflict") return { kind };
  if (kind === "InvalidName") return { kind };
  if (kind === "NotFound") return { kind };
  if (kind === "Io") return { kind };
  return null;
}

function asDeleteError(cause: unknown): DeleteError | null {
  if (typeof cause !== "object" || cause === null || !("kind" in cause)) {
    return null;
  }
  const { kind } = cause;
  if (kind === "NotFound") return { kind };
  if (kind === "Io") return { kind };
  return null;
}

/**
 * Имя новой заметки по умолчанию — на языке интерфейса в момент создания
 * (фича 010): это заготовка, которую пользователь тут же может заменить.
 */
function defaultNewNoteName(): string {
  return currentI18n().t("tree.defaultNoteName");
}
/** Имя новой доски по умолчанию (фича 002, FR-001). */
export function defaultNewBoardName(): string {
  return currentI18n().t("tree.defaultBoardName");
}

/**
 * Подтверждение перед удалением (T090, FR-027, FR-029): сначала спрашиваем у
 * ядра, какие заметки останутся с битой ссылкой, и показываем именно их.
 * Если ссылок нет — обычное подтверждение удаления, без пустого диалога
 * «затронуто 0 заметок» (US5 AS2).
 *
 * `requestDelete` ниже по-прежнему не знает, как получено согласие, — только
 * его итог true/false, поэтому диалог со списком подменяется через проп
 * `confirmDelete` без изменения остального файла.
 */
function makeDefaultConfirmDelete(client: VaultClient) {
  return async (note: Note): Promise<boolean> => {
    const affected = await client.backlinks(note.path);
    if (affected.length === 0) {
      return window.confirm(currentI18n().t("tree.confirm.delete", { name: note.name }));
    }
    const list = affected.map((n) => `  • ${n.path}`).join("\n");
    return window.confirm(currentI18n().t("tree.confirm.deleteLinked", { name: note.name, list }));
  };
}

/**
 * Подтверждение перед переименованием (T090, FR-028, FR-029). Переименование
 * само по себе не деструктивно (FR-010), поэтому без входящих ссылок оно
 * проходит молча; но содержимое ссылающихся заметок приложение не переписывает,
 * так что при непустом списке пользователь обязан увидеть, что именно сломается.
 */
function makeDefaultConfirmRename(client: VaultClient) {
  return async (note: Note, nextPath: string): Promise<boolean> => {
    const affected = await client.backlinks(note.path);
    if (affected.length === 0) return true;
    const list = affected.map((n) => `  • ${n.path}`).join("\n");
    return window.confirm(
      currentI18n().t("tree.confirm.renameLinked", { from: note.path, to: nextPath, list }),
    );
  };
}

/**
 * Путь открытого файла — строкам дерева, чтобы отметить активную (макет §
 * Sidebar). Контекст, а не проп: между `VaultTreeActions` и строкой заметки
 * два промежуточных компонента, которым до открытого файла нет дела, и
 * протаскивание через них добавило бы им по параметру ни за чем.
 */
const OpenPathContext = createContext<string | null>(null);

/**
 * Действия дерева, которым нужна рабочая область, — сейчас одно: вставить
 * ссылку на заметку в открытую заметку. Контекст по той же причине, что
 * `OpenPathContext`: строки лежат на два уровня глубже, чем пропы дерева.
 */
const TreeExtrasContext = createContext<{
  onInsertLink?: (note: Note) => void;
  /** История файла (фича 011, FR-009). */
  onFileHistory?: (note: Note) => void;
  /** Пути с неразрешённым конфликтом получения или Syncthing (фича 011). */
  conflictPaths?: ReadonlySet<string>;
}>({});

/** Путь в буфер обмена с отчётом — одинаково для заметки и папки. */
function copyPathItem(
  t: Translate,
  path: string,
  notify: (message: string) => void,
): ContextMenuItem {
  return {
    label: t("tree.menu.copyPath"),
    run: async () => {
      const failure = await copyText(path);
      notify(failure ?? t("tree.menu.copied", { path }));
    },
  };
}

/**
 * Что нужно самому состоянию действий — им пользуется `useVaultTreeActions`.
 *
 * Отдельно от пропов компонента: с тех пор как кнопка создания переехала в
 * шапку панели (макет «Typsidian Доски» § Sidebar), состояние заводит
 * `Sidebar`, а дерево получает его готовым. Держать оба набора в одном
 * интерфейсе значило бы требовать от дерева `client`, который ему не нужен.
 */
export interface VaultTreeActionsOptions {
  client: VaultClient;
  /** Вызывается после успешного create/rename/delete — обычно `useVault().refresh`. */
  onNotesChanged: () => void | Promise<void>;
  /**
   * Файл создан. Нужен доскам: увидеть доску можно только на полотне, поэтому
   * созданная доска сразу открывается (`quickstart.md` Сценарий 1, шаг 2), а
   * число действий до начала рисования остаётся в бюджете SC-002.
   */
  onCreated?: (path: string) => void;
  /** FR-013, FR-014: конкретная причина отказа — передать в `ErrorBanner`. */
  onError: (error: VaultOperationError) => void;
  /**
   * Переопределяет подтверждение переименования (FR-028) — например, на
   * `BacklinksConfirmDialog` вместо встроенного текстового. По умолчанию —
   * `makeDefaultConfirmRename`, спрашивающий `backlinks()` у ядра.
   */
  confirmRename?: (note: Note, nextPath: string) => boolean | Promise<boolean>;
  /** То же для удаления (FR-027) — см. `makeDefaultConfirmDelete`. */
  confirmDelete?: (note: Note) => boolean | Promise<boolean>;

  /**
   * Пути папок, помеченных как разделы (фича 006, FR-014).
   *
   * Приходят снаружи, а не читаются здесь: тот же набор нужен панели настроек,
   * и второе место, спрашивающее ядро о разделах, разошлось бы с первым при
   * первой же правке.
   */
  sections?: string[];
  /**
   * Пометить папку разделом или снять пометку (FR-013). `undefined` — vault не
   * открыт либо возможность не подключена: пункт меню тогда не показывается,
   * а не показывается неработающим.
   */
  onToggleSection?: (folder: string, marked: boolean) => void | Promise<void>;
}

/** Что нужно самому дереву: список, открытие записи и готовое состояние. */
export interface VaultTreeActionsProps {
  notes: Note[];
  onOpenNote: (note: Note) => void;
  /** Какой файл сейчас открыт — он выделяется в дереве (FR-004 макета). */
  openPath?: string | null;
  actions: VaultTreeActionsController;
  /**
   * Вставить ссылку `[[…]]` на заметку в открытую заметку — пункт контекстного
   * меню. Не передан — открытой текстовой заметки нет, и пункт не показывается.
   */
  onInsertLink?: ((note: Note) => void) | undefined;
  onFileHistory?: ((note: Note) => void) | undefined;
  conflictPaths?: ReadonlySet<string> | undefined;
}

type MenuTarget =
  | { kind: "note"; note: Note }
  | { kind: "folder"; path: string };

interface ActionsState {
  renamingPath: string | null;
  renameDraft: string;
  creatingInFolder: string | null;
  createDraft: string;
  /**
   * Заготовка, выбранная в меню создания (фича 003, FR-020). `null` — обычная
   * пустая заметка. Живёт рядом с черновиком имени, потому что относится к тому
   * же незавершённому действию: пользователь уже выбрал заготовку, но ещё не
   * назвал файл.
   */
  createTemplate: Template | null;
  menu: MenuTarget | null;
  /** Путь заметки/будущей заметки, к которой сейчас идёт запрос к client. */
  pendingPath: string | null;
}

const INITIAL_STATE: ActionsState = {
  renamingPath: null,
  renameDraft: "",
  creatingInFolder: null,
  // Заготовка имени ставится в `beginCreate` — на языке интерфейса того момента.
  createDraft: "",
  createTemplate: null,
  menu: null,
  pendingPath: null,
};

/**
 * Инкапсулирует UI-состояние create/rename/delete (T042) поверх
 * `VaultClient`. Вызовы к ядру (`performCreate`/`performRename`/
 * `performDelete`) выделены в отдельные функции по требованию tasks.md —
 * T090 расширяет их не изменением самого вызова client, а подменой
 * `confirmRename`/`confirmDelete` (шаг ПЕРЕД вызовом), который здесь уже
 * является самостоятельной точкой расширения.
 */
export function useVaultTreeActions({
  client,
  onNotesChanged,
  onCreated,
  onError,
  confirmRename,
  confirmDelete,
  sections = [],
  onToggleSection,
}: VaultTreeActionsOptions) {
  const [state, setState] = useState<ActionsState>(INITIAL_STATE);
  // Значения по умолчанию зависят от `client` (им нужен `backlinks()`),
  // поэтому вычисляются здесь, а не в списке параметров.
  const effectiveConfirmRename = useMemo(
    () => confirmRename ?? makeDefaultConfirmRename(client),
    [confirmRename, client],
  );
  const effectiveConfirmDelete = useMemo(
    () => confirmDelete ?? makeDefaultConfirmDelete(client),
    [confirmDelete, client],
  );

  const openMenu = useCallback((target: MenuTarget) => {
    setState((prev) => ({ ...prev, menu: target }));
  }, []);

  const closeMenu = useCallback(() => {
    setState((prev) => ({ ...prev, menu: null }));
  }, []);

  const beginRename = useCallback((note: Note) => {
    setState((prev) => ({
      ...prev,
      renamingPath: note.path,
      renameDraft: note.name,
      menu: null,
    }));
  }, []);

  const cancelRename = useCallback(() => {
    setState((prev) => ({ ...prev, renamingPath: null, renameDraft: "" }));
  }, []);

  const setRenameDraft = useCallback((value: string) => {
    setState((prev) => ({ ...prev, renameDraft: value }));
  }, []);

  // Прямой вызов `client.rename` (contracts/vault-backend-trait.md) — по
  // требованию UI-задачи никакой повторной валидации имени сверх того, что
  // нужно для UX: ядро остаётся источником истины по допустимости.
  const performRename = useCallback(
    async (note: Note, nextPath: string) => {
      if (state.pendingPath !== null) return;
      setState((prev) => ({ ...prev, pendingPath: note.path }));
      try {
        await client.rename(note.path, nextPath);
        setState((prev) => ({
          ...prev,
          pendingPath: null,
          renamingPath: null,
          renameDraft: "",
        }));
        await onNotesChanged();
      } catch (cause) {
        setState((prev) => ({ ...prev, pendingPath: null }));
        const error = asRenameError(cause);
        onError({ operation: "rename", error: error ?? { kind: "Io" } });
      }
    },
    [client, onNotesChanged, onError, state.pendingPath],
  );

  const submitRename = useCallback(
    async (note: Note) => {
      if (state.pendingPath !== null) return;
      const name = state.renameDraft.trim();
      if (name === "" || name === note.name) {
        cancelRename();
        return;
      }
      const nextPath = joinPath(folderOf(note.path), name);
      const allowed = await effectiveConfirmRename(note, nextPath);
      if (!allowed) {
        cancelRename();
        return;
      }
      await performRename(note, nextPath);
    },
    [
      state.pendingPath,
      state.renameDraft,
      effectiveConfirmRename,
      performRename,
      cancelRename,
    ],
  );

  /**
   * `kind` определяет только имя по умолчанию в поле ввода. Что именно будет
   * создано, решает расширение в итоговом имени — то же правило, что у ядра
   * (`classify`): если пользователь переименует черновик из `.excalidraw` в
   * `.typ`, получится заметка, и наоборот.
   *
   * `template` (фича 003, FR-020, FR-022) — заготовка, выбранная вторым уровнем
   * меню создания. Она **не** меняет ни путь создания, ни правила имён: имя и
   * расположение задаются тем же инлайн-вводом, что и у пустой заметки. Её
   * содержимое дописывается уже после создания файла (`performCreate`).
   * `null` — обычная пустая заметка (FR-024).
   */
  const beginCreate = useCallback(
    (
      folderPath: string,
      kind: "note" | "board" = "note",
      template: Template | null = null,
    ) => {
      setState((prev) => ({
        ...prev,
        creatingInFolder: folderPath,
        createDraft:
          kind === "board" ? defaultNewBoardName() : defaultNewNoteName(),
        createTemplate: template,
        menu: null,
      }));
    },
    [],
  );

  const cancelCreate = useCallback(() => {
    setState((prev) => ({
      ...prev,
      creatingInFolder: null,
      createDraft: "",
      createTemplate: null,
    }));
  }, []);

  const setCreateDraft = useCallback((value: string) => {
    setState((prev) => ({ ...prev, createDraft: value }));
  }, []);

  // Прямой вызов `client.create` — см. комментарий у `performRename`.
  const performCreate = useCallback(
    async (path: string, template: Template | null) => {
      if (state.pendingPath !== null) return;
      setState((prev) => ({ ...prev, pendingPath: path }));
      try {
        // Доска создаётся своим вызовом: `create` оставил бы пустой файл, а
        // пустой файл доски не разбирается движком и читается ядром как
        // `Unreadable` (FR-017 фичи 002). Вид определяет ядро по пути, а не эта
        // ветка: имени в дереве ещё нет, поэтому его приходится спросить
        // (принцип I).
        const board = (await client.fileKind(path)) === "Board";
        if (board) await client.createBoard(path);
        else await client.create(path);

        // Заготовка применяется **после** создания файла и только к заметке:
        // выбрав заготовку и назвав файл `.excalidraw`, пользователь сказал
        // «доска», и содержимое заготовки туда не поедет (фича 003, FR-021).
        const filled = !board && template !== null;
        if (filled) await fillFromTemplate(client, path, template);

        setState((prev) => ({
          ...prev,
          pendingPath: null,
          creatingInFolder: null,
          createDraft: defaultNewNoteName(),
          createTemplate: null,
        }));
        await onNotesChanged();
        // Доска открывается сразу: её содержимое видно только на полотне.
        // Пустая заметка — нет: она видна в дереве, и открывать её за
        // пользователя значило бы решить за него, чем он занят. Заметка,
        // созданная по заготовке, — да: он уже сказал, чем занят, выбрав её.
        if (board || filled) onCreated?.(path);
      } catch (cause) {
        setState((prev) => ({ ...prev, pendingPath: null }));
        const error = asCreateError(cause);
        onError({ operation: "create", error: error ?? { kind: "Io" } });
      }
    },
    [client, onNotesChanged, onCreated, onError, state.pendingPath],
  );

  const submitCreate = useCallback(
    async (folderPath: string) => {
      if (state.pendingPath !== null) return;
      const name = state.createDraft.trim();
      if (name === "") {
        cancelCreate();
        return;
      }
      await performCreate(joinPath(folderPath, name), state.createTemplate);
    },
    [
      state.pendingPath,
      state.createDraft,
      state.createTemplate,
      performCreate,
      cancelCreate,
    ],
  );

  // Прямой вызов `client.delete` — см. комментарий у `performRename`.
  const performDelete = useCallback(
    async (note: Note) => {
      if (state.pendingPath !== null) return;
      setState((prev) => ({ ...prev, pendingPath: note.path, menu: null }));
      try {
        await client.delete(note.path);
        setState((prev) => ({ ...prev, pendingPath: null }));
        await onNotesChanged();
      } catch (cause) {
        setState((prev) => ({ ...prev, pendingPath: null }));
        const error = asDeleteError(cause);
        onError({ operation: "delete", error: error ?? { kind: "Io" } });
      }
    },
    [client, onNotesChanged, onError, state.pendingPath],
  );

  const requestDelete = useCallback(
    async (note: Note) => {
      if (state.pendingPath !== null) return;
      setState((prev) => ({ ...prev, menu: null }));
      const allowed = await effectiveConfirmDelete(note);
      if (!allowed) return;
      await performDelete(note);
    },
    [state.pendingPath, effectiveConfirmDelete, performDelete],
  );

  return {
    state,
    openMenu,
    closeMenu,
    beginRename,
    cancelRename,
    setRenameDraft,
    submitRename,
    beginCreate,
    cancelCreate,
    setCreateDraft,
    submitCreate,
    requestDelete,
    sections,
    onToggleSection,
  };
}

type VaultTreeActionsController = ReturnType<typeof useVaultTreeActions>;

function nodeKey(node: TreeNode): string {
  return node.kind === "folder" ? `folder:${node.path}` : `note:${node.note.path}`;
}

function InlineTextInput({
  value,
  ariaLabel,
  disabled,
  onChange,
  onCommit,
  onCancel,
}: {
  value: string;
  ariaLabel: string;
  disabled: boolean;
  onChange: (value: string) => void;
  onCommit: () => void;
  onCancel: () => void;
}): React.JSX.Element {
  return (
    <input
      type="text"
      value={value}
      disabled={disabled}
      autoFocus
      aria-label={ariaLabel}
      onChange={(event) => onChange(event.target.value)}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          onCommit();
        } else if (event.key === "Escape") {
          event.preventDefault();
          onCancel();
        }
      }}
      onBlur={onCommit}
    />
  );
}

/**
 * Признак вида файла в строке дерева (FR-004 фичи 002, SC-001): доска отличима
 * от текстовой заметки с одного взгляда, без открытия файла. Вид приходит из
 * ядра вместе с записью (`Note.kind`), а не выводится из расширения здесь
 * (принцип I).
 *
 * Вид несёт **форма** иконки, а состояние — её цвет: активная строка красит
 * иконку в оранжевый (шаблон § Sidebar), и если бы цветом обозначался ещё и
 * вид, доска и открытая заметка выглядели бы одинаково.
 */
function FileKindMark({ note }: { note: Note }): React.JSX.Element {
  return (
    <Icon
      name={isBoard(note) ? "board" : "file"}
      size={13}
      className={isBoard(note) ? "ts-kind ts-kind--board" : "ts-kind"}
    />
  );
}

/** Имя без расширения — расширение рисуется отдельным хвостом. */
function stemOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/** Расширение вместе с точкой; пустая строка, если его нет. */
function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot) : "";
}

function NoteRow({
  note,
  actions,
  onOpenNote,
}: {
  note: Note;
  actions: VaultTreeActionsController;
  onOpenNote: (note: Note) => void;
}): React.JSX.Element {
  const menu = actions.state.menu;
  const isRenaming = actions.state.renamingPath === note.path;
  const isMenuOpen = menu !== null && menu.kind === "note" && menu.note.path === note.path;
  const isBusy = actions.state.pendingPath !== null;
  const isOpen = useContext(OpenPathContext) === note.path.toString();
  const contextMenu = useContextMenu();
  const { onInsertLink, onFileHistory, conflictPaths } = useContext(TreeExtrasContext);
  const { t } = useI18n();
  const hasConflict = conflictPaths?.has(note.path.toString()) ?? false;

  if (isRenaming) {
    return (
      <li role="treeitem">
        <InlineTextInput
          value={actions.state.renameDraft}
          ariaLabel={t("tree.rename.label", { name: note.name })}
          disabled={isBusy}
          onChange={actions.setRenameDraft}
          onCommit={() => void actions.submitRename(note)}
          onCancel={actions.cancelRename}
        />
      </li>
    );
  }

  return (
    <li
      role="treeitem"
      className="ts-tree__file"
      onContextMenu={(event) => {
        if (isBusy) return;
        contextMenu.open(event, [
          { label: t("tree.menu.open"), run: () => onOpenNote(note) },
          ...(onInsertLink && !isBoard(note) && !isOpen
            ? [{ label: t("tree.menu.insertLink"), run: () => onInsertLink(note) }]
            : []),
          ...(onFileHistory
            ? [{ label: t("sync.tree.fileHistory"), run: () => onFileHistory(note) }]
            : []),
          { kind: "separator" },
          copyPathItem(t, note.path.toString(), contextMenu.notify),
          { label: t("common.rename"), run: () => actions.beginRename(note) },
          { kind: "separator" },
          { label: t("tree.menu.deleteEllipsis"), danger: true, run: () => actions.requestDelete(note) },
        ]);
      }}
    >
      <button
        type="button"
        onClick={() => onOpenNote(note)}
        disabled={isBusy}
        aria-current={isOpen ? "true" : undefined}
      >
        <FileKindMark note={note} />
        <span className="ts-tree__name">{stemOf(note.name)}</span>
      {/* Расширение отдельным приглушённым хвостом — макет досок § Sidebar:
          имя обрезается многоточием, а вид файла остаётся виден. */}
      <span className="ts-tree__ext">{extensionOf(note.name)}</span>
        {/* Hi-vis жёлтая точка — только про несохранённые правки (handoff §
            Design Tokens). Признак приходит из ядра вместе с записью, а не
            выводится здесь из состояния редактора. */}
        {note.has_unsaved_edits && (
          <span className="ts-dot-unsaved" title={t("tree.unsaved")} />
        )}
        {/* Конфликт получения или копия Syncthing (фича 011): факт приходит
            из ядра списком конфликтов, а не выводится здесь из имён файлов. */}
        {hasConflict && (
          <span className="ts-tree__conflict" title={t("sync.tree.conflict")}>
            <span className="sr-only">{t("sync.tree.conflict")}</span>!
          </span>
        )}
      </button>
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={isMenuOpen}
        aria-label={t("tree.noteActions", { name: note.name })}
        disabled={isBusy}
        onClick={() =>
          isMenuOpen ? actions.closeMenu() : actions.openMenu({ kind: "note", note })
        }
      >
        ⋯
      </button>
      {isMenuOpen && (
        <ul
          role="menu"
          onKeyDown={(event) => {
            if (event.key === "Escape") actions.closeMenu();
          }}
        >
          <li role="none">
            <button type="button" role="menuitem" onClick={() => actions.beginRename(note)}>
              {t("common.rename")}
            </button>
          </li>
          <li role="none">
            <button
              type="button"
              role="menuitem"
              onClick={() => void actions.requestDelete(note)}
            >
              {t("common.delete")}
            </button>
          </li>
        </ul>
      )}
    </li>
  );
}

function FolderRow({
  folder,
  actions,
  onOpenNote,
}: {
  folder: TreeFolder;
  actions: VaultTreeActionsController;
  onOpenNote: (note: Note) => void;
}): React.JSX.Element {
  // Верхний уровень раскрыт по умолчанию, как и в VaultTree.tsx (SC-002).
  const [expanded, setExpanded] = useState(true);
  const menu = actions.state.menu;
  const isMenuOpen = menu !== null && menu.kind === "folder" && menu.path === folder.path;
  const isCreatingHere = actions.state.creatingInFolder === folder.path;
  // Помечена ли папка как раздел (фича 006, FR-014). Сравнение по строке
  // пути — ровно тому, что хранит ядро; выводить признак из имени папки
  // нельзя, разделом её делает пометка, а не название.
  const isSection = actions.sections.includes(folder.path);
  // Папка принудительно раскрыта, пока в ней создаётся заметка через меню —
  // иначе новый инлайн-инпут появлялся бы в свёрнутой, невидимой ветке.
  const showChildren = expanded || isCreatingHere;
  const isBusy = actions.state.pendingPath !== null;
  const contextMenu = useContextMenu();
  const { t } = useI18n();

  return (
    <li
      role="treeitem"
      aria-expanded={showChildren}
      className="ts-tree__folder"
      onContextMenu={(event) => {
        if (isBusy) return;
        contextMenu.open(event, [
          { label: t("tree.menu.createNoteHere"), run: () => actions.beginCreate(folder.path) },
          {
            label: t("tree.menu.createBoardHere"),
            run: () => actions.beginCreate(folder.path, "board"),
          },
          { kind: "separator" },
          ...(actions.onToggleSection
            ? [
                {
                  label: isSection ? t("tree.section.unmark") : t("tree.section.mark"),
                  run: () => actions.onToggleSection?.(folder.path, !isSection),
                },
              ]
            : []),
          copyPathItem(t, folder.path, contextMenu.notify),
        ]);
      }}
    >
      {/* Строка папки по шаблону: иконка папки и имя. Шеврон в шаблоне не
          нарисован — там папки декоративные заголовки, а здесь они
          сворачиваются, и управляющий элемент без признака состояния был бы
          хуже точного совпадения с картинкой. */}
      <button type="button" onClick={() => setExpanded((v) => !v)}>
        <Icon
          name="chevron"
          size={11}
          className={showChildren ? "ts-tree__twist ts-tree__twist--open" : "ts-tree__twist"}
        />
        <Icon name="folder" size={14} className="ts-tree__folder-icon" />
        <span className="ts-tree__name">{folder.name || "/"}</span>
        {isSection && (
          <span className="ts-tree__section-badge" title={t("tree.section.rootTitle")}>
            {t("tree.section.badge")}
          </span>
        )}
      </button>
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={isMenuOpen}
        aria-label={t("tree.folderActions", { name: folder.name || t("tree.rootFolder") })}
        disabled={isBusy}
        onClick={() =>
          isMenuOpen
            ? actions.closeMenu()
            : actions.openMenu({ kind: "folder", path: folder.path })
        }
      >
        ⋯
      </button>
      {isMenuOpen && (
        <ul
          role="menu"
          onKeyDown={(event) => {
            if (event.key === "Escape") actions.closeMenu();
          }}
        >
          <li role="none">
            <button
              type="button"
              role="menuitem"
              onClick={() => actions.beginCreate(folder.path)}
            >
              {t("tree.menu.createNote")}
            </button>
          </li>
          {actions.onToggleSection && (
            <li role="none">
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  void actions.onToggleSection?.(folder.path, !isSection);
                  actions.closeMenu();
                }}
              >
                {isSection ? t("tree.section.unmark") : t("tree.section.mark")}
              </button>
            </li>
          )}
        </ul>
      )}
      {showChildren && (
        <ul role="group">
          {isCreatingHere && (
            <li role="treeitem">
              <InlineTextInput
                value={actions.state.createDraft}
                ariaLabel={t("tree.create.nameLabel")}
                disabled={isBusy}
                onChange={actions.setCreateDraft}
                onCommit={() => void actions.submitCreate(folder.path)}
                onCancel={actions.cancelCreate}
              />
            </li>
          )}
          {folder.children.map((child) => (
            <TreeNodeRow key={nodeKey(child)} node={child} actions={actions} onOpenNote={onOpenNote} />
          ))}
        </ul>
      )}
    </li>
  );
}

function TreeNodeRow({
  node,
  actions,
  onOpenNote,
}: {
  node: TreeNode;
  actions: VaultTreeActionsController;
  onOpenNote: (note: Note) => void;
}): React.JSX.Element {
  if (node.kind === "note") {
    return <NoteRow note={node.note} actions={actions} onOpenNote={onOpenNote} />;
  }
  return <FolderRow folder={node} actions={actions} onOpenNote={onOpenNote} />;
}

/**
 * Дерево vault с UI-действиями create/rename/delete (T042): контекстное
 * меню (правый клик или кнопка «⋯») на заметке даёт «Переименовать»/
 * «Удалить», на папке/корне — «Создать заметку»; переименование происходит
 * инлайн прямо в дереве (Enter/blur — подтвердить, Escape — отменить).
 * Открытие заметки (`onOpenNote`) и построение иерархии (`buildTree`)
 * переиспользуют ту же логику, что и read-only `VaultTree` из US1.
 */
export function VaultTreeActions({
  notes,
  onOpenNote,
  openPath = null,
  actions,
  onInsertLink,
  onFileHistory,
  conflictPaths,
}: VaultTreeActionsProps): React.JSX.Element {
  const { t } = useI18n();
  const tree = useMemo(() => buildTree(notes), [notes]);
  const isCreatingAtRoot = actions.state.creatingInFolder === "";
  const isBusy = actions.state.pendingPath !== null;
  const extras = useMemo(
    () => ({
      ...(onInsertLink ? { onInsertLink } : {}),
      ...(onFileHistory ? { onFileHistory } : {}),
      ...(conflictPaths ? { conflictPaths } : {}),
    }),
    [onInsertLink, onFileHistory, conflictPaths],
  );

  return (
    <OpenPathContext.Provider value={openPath}>
    <TreeExtrasContext.Provider value={extras}>
    <div>
      {notes.length === 0 && !isCreatingAtRoot ? (
        <p role="status">{t("tree.emptyNotesOrBoards")}</p>
      ) : (
        <ul role="tree" aria-label={t("tree.label")} className="ts-tree__root">
          {/* Заголовок группы из шаблона § Sidebar. `presentation`, потому что
              для дерева это подпись, а не узел: своим `treeitem` он добавил бы
              в обход с клавиатуры строку, которую нельзя открыть. */}
          <li className="ts-tree__eyebrow" role="presentation">
            Vault
          </li>
          {isCreatingAtRoot && (
            <li role="treeitem">
              <InlineTextInput
                value={actions.state.createDraft}
                ariaLabel={t("tree.create.nameOrBoardLabel")}
                disabled={isBusy}
                onChange={actions.setCreateDraft}
                onCommit={() => void actions.submitCreate("")}
                onCancel={actions.cancelCreate}
              />
            </li>
          )}
          {tree.map((node) => (
            <TreeNodeRow key={nodeKey(node)} node={node} actions={actions} onOpenNote={onOpenNote} />
          ))}
        </ul>
      )}
    </div>
    </TreeExtrasContext.Provider>
    </OpenPathContext.Provider>
  );
}
