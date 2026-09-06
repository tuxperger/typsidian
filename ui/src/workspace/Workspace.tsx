"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
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

import {
  DEFAULT_NEW_BOARD_NAME,
  VaultTreeActions,
} from "../vault-tree/VaultTreeActions.js";
import type {
  CreateError,
  Note,
} from "../wasm-core/typsidian_core.js";
import { BoardCanvas } from "../boards/BoardCanvas.js";
import { BoardConflictDialog } from "../boards/BoardConflictDialog.js";
import { useBoardConflict } from "../boards/useBoardConflict.js";
import { useBoardDocument } from "../boards/useBoardDocument.js";
import { isBoard } from "../vault-tree/fileKind.js";
import { NoteEditor } from "../editor/NoteEditor.js";
import { PreviewPane } from "../preview/PreviewPane.js";
import type { PreviewClient } from "../preview-client.js";

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
   */
  settings?: React.ReactNode;
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

  // Конфликты приходят событиями из ядра; решение принимает пользователь в
  // диалоге, а не эта оболочка (FR-021).
  const [clientForConflicts, setClientForConflicts] = useState<
    Parameters<typeof useConflictResolution>[0]
  >(null);
  const conflicts = useConflictResolution(clientForConflicts);

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
  });

  // `useConflictResolution` создаётся до того, как появится backend, поэтому
  // связываем их, как только vault открыт.
  if (state.client && clientForConflicts !== state.client) {
    setClientForConflicts(state.client);
  }

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
      <header className="ts-header">
        <div className="ts-header__group">
          <span className="ts-wordmark">
            <span className="ts-wordmark__slash">/</span>typsidian
          </span>
          <span className="ts-divider" />
          {openNote && <span className="ts-crumb">{openNote.path}</span>}
        </div>
        <div className="ts-header__group">
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
        <AccessStateBanner accessState={state.accessState} onReopenVault={openVault} />
        <BulkChangeIndicator active={state.bulkChangeDepth > 0} />
        <ErrorBanner error={operationError} onDismiss={() => setOperationError(null)} />
      </div>

      <div className="ts-body">
        <aside className="ts-sidebar">
          <nav className="ts-tree" aria-label="Заметки">
            <VaultTreeActions
              client={state.client}
              notes={state.notes}
              onOpenNote={openEntry}
              onNotesChanged={refresh}
              onCreated={(path) => void openPath(path)}
              onError={setOperationError}
            />
          </nav>
          <div className="ts-sidebar__footer">
            {plural(state.notes.length, "заметка", "заметки", "заметок")} ·{" "}
            {plural(folderCount, "папка", "папки", "папок")}
          </div>
        </aside>

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
            />

            <PreviewPane
              client={preview}
              notePath={openNote?.path ?? null}
              text={editorText}
            />
          </>
        )}
      </div>

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
            <div className="ts-sheet__body">
              {settings}
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

const RU_PLURAL = new Intl.PluralRules("ru-RU");

/** «1 заметка», «3 заметки», «9 заметок» — иначе подвал дерева врёт грамматикой. */
function plural(count: number, one: string, few: string, many: string): string {
  switch (RU_PLURAL.select(count)) {
    case "one":
      return `${count} ${one}`;
    case "few":
      return `${count} ${few}`;
    default:
      return `${count} ${many}`;
  }
}
