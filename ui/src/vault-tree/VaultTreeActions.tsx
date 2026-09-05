import { useCallback, useMemo, useState } from "react";
import type {
  CreateError,
  DeleteError,
  Note,
  RenameError,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { buildTree, type TreeFolder, type TreeNode } from "./buildTree.js";
import type { VaultOperationError } from "./ErrorBanner.js";
import { isBoard } from "./fileKind.js";

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

const DEFAULT_NEW_NOTE_NAME = "Новая заметка.typ";
/** Имя новой доски по умолчанию (фича 002, FR-001). */
export const DEFAULT_NEW_BOARD_NAME = "Новая доска.excalidraw";

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
      return window.confirm(
        `Удалить заметку «${note.name}»? Это действие нельзя отменить из интерфейса.`,
      );
    }
    const list = affected.map((n) => `  • ${n.path}`).join("\n");
    return window.confirm(
      `После удаления «${note.name}» останутся с битой ссылкой:\n${list}\n\nУдалить всё равно?`,
    );
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
      `После переименования «${note.path}» → «${nextPath}» эти заметки продолжат ссылаться на старое имя, и их ссылки станут битыми:\n${list}\n\nПереименовать всё равно?`,
    );
  };
}

export interface VaultTreeActionsProps {
  client: VaultClient;
  notes: Note[];
  onOpenNote: (note: Note) => void;
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
}

type MenuTarget =
  | { kind: "note"; note: Note }
  | { kind: "folder"; path: string };

interface ActionsState {
  renamingPath: string | null;
  renameDraft: string;
  creatingInFolder: string | null;
  createDraft: string;
  menu: MenuTarget | null;
  /** Путь заметки/будущей заметки, к которой сейчас идёт запрос к client. */
  pendingPath: string | null;
}

const INITIAL_STATE: ActionsState = {
  renamingPath: null,
  renameDraft: "",
  creatingInFolder: null,
  createDraft: DEFAULT_NEW_NOTE_NAME,
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
}: Pick<
  VaultTreeActionsProps,
  | "client"
  | "onNotesChanged"
  | "onCreated"
  | "onError"
  | "confirmRename"
  | "confirmDelete"
>) {
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
   */
  const beginCreate = useCallback((folderPath: string, kind: "note" | "board" = "note") => {
    setState((prev) => ({
      ...prev,
      creatingInFolder: folderPath,
      createDraft: kind === "board" ? DEFAULT_NEW_BOARD_NAME : DEFAULT_NEW_NOTE_NAME,
      menu: null,
    }));
  }, []);

  const cancelCreate = useCallback(() => {
    setState((prev) => ({ ...prev, creatingInFolder: null, createDraft: "" }));
  }, []);

  const setCreateDraft = useCallback((value: string) => {
    setState((prev) => ({ ...prev, createDraft: value }));
  }, []);

  // Прямой вызов `client.create` — см. комментарий у `performRename`.
  const performCreate = useCallback(
    async (path: string) => {
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
        setState((prev) => ({
          ...prev,
          pendingPath: null,
          creatingInFolder: null,
          createDraft: DEFAULT_NEW_NOTE_NAME,
        }));
        await onNotesChanged();
        // Доска открывается сразу: её содержимое видно только на полотне.
        // Заметка — нет: она видна в дереве, и открывать её за пользователя
        // значило бы решить за него, чем он занят.
        if (board) onCreated?.(path);
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
      await performCreate(joinPath(folderPath, name));
    },
    [state.pendingPath, state.createDraft, performCreate, cancelCreate],
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
 */
function FileKindMark({ note }: { note: Note }): React.JSX.Element {
  const board = isBoard(note);
  return (
    <span className={board ? "ts-kind ts-kind--board" : "ts-kind"} aria-hidden="true">
      {board ? "◈" : "▪"}
    </span>
  );
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

  if (isRenaming) {
    return (
      <li role="treeitem">
        <InlineTextInput
          value={actions.state.renameDraft}
          ariaLabel={`Новое имя для заметки ${note.name}`}
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
      onContextMenu={(event) => {
        event.preventDefault();
        actions.openMenu({ kind: "note", note });
      }}
    >
      <button type="button" onClick={() => onOpenNote(note)} disabled={isBusy}>
        <FileKindMark note={note} />
        {note.name}
      </button>
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={isMenuOpen}
        aria-label={`Действия с заметкой ${note.name}`}
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
              Переименовать
            </button>
          </li>
          <li role="none">
            <button
              type="button"
              role="menuitem"
              onClick={() => void actions.requestDelete(note)}
            >
              Удалить
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
  // Папка принудительно раскрыта, пока в ней создаётся заметка через меню —
  // иначе новый инлайн-инпут появлялся бы в свёрнутой, невидимой ветке.
  const showChildren = expanded || isCreatingHere;
  const isBusy = actions.state.pendingPath !== null;

  return (
    <li role="treeitem" aria-expanded={showChildren}>
      <button type="button" onClick={() => setExpanded((v) => !v)}>
        {showChildren ? "▾" : "▸"} {folder.name || "/"}
      </button>
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={isMenuOpen}
        aria-label={`Действия с папкой ${folder.name || "корень"}`}
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
              Создать заметку
            </button>
          </li>
        </ul>
      )}
      {showChildren && (
        <ul role="group">
          {isCreatingHere && (
            <li role="treeitem">
              <InlineTextInput
                value={actions.state.createDraft}
                ariaLabel="Имя новой заметки"
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
  client,
  notes,
  onOpenNote,
  onNotesChanged,
  onCreated,
  onError,
  confirmRename,
  confirmDelete,
}: VaultTreeActionsProps): React.JSX.Element {
  const actions = useVaultTreeActions({
    client,
    onNotesChanged,
    onError,
    ...(onCreated ? { onCreated } : {}),
    ...(confirmRename ? { confirmRename } : {}),
    ...(confirmDelete ? { confirmDelete } : {}),
  });
  const tree = useMemo(() => buildTree(notes), [notes]);
  const isCreatingAtRoot = actions.state.creatingInFolder === "";
  const isBusy = actions.state.pendingPath !== null;

  return (
    <div>
      <div role="toolbar" aria-label="Действия vault">
        <button type="button" disabled={isBusy} onClick={() => actions.beginCreate("")}>
          + Новая заметка
        </button>
        {/* FR-001 фичи 002: доска создаётся в vault наравне с заметкой. Без
            этой кнопки операция ядра `createBoard` недостижима из приложения. */}
        <button
          type="button"
          disabled={isBusy}
          onClick={() => actions.beginCreate("", "board")}
        >
          + Новая доска
        </button>
      </div>
      {notes.length === 0 && !isCreatingAtRoot ? (
        <p role="status">Vault пуст — создайте первую заметку или доску.</p>
      ) : (
        <ul role="tree" aria-label="Дерево заметок">
          {isCreatingAtRoot && (
            <li role="treeitem">
              <InlineTextInput
                value={actions.state.createDraft}
                ariaLabel="Имя новой заметки или доски"
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
  );
}
