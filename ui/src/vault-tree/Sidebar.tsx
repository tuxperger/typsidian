"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import type { VaultClient } from "../vault-client.js";
import type { Note, PathChange, Template } from "../wasm-core/typsidian_core.js";
import { BacklinksConfirmDialog } from "./BacklinksConfirmDialog.js";
import { MoveNotice } from "./MoveNotice.js";
import { MoveToDialog } from "./MoveToDialog.js";
import { TreeDragGhost } from "./TreeDragGhost.js";
import { useMoveEntry, type MoveEntryRef } from "./useMoveEntry.js";
import { useTreeDrag } from "./useTreeDrag.js";
import { CreateFileMenu, type CreateKind } from "./CreateFileMenu.js";
import type { VaultOperationError } from "./ErrorBanner.js";
import { VaultTreeActions, useVaultTreeActions } from "./VaultTreeActions.js";
import { useVaultSettings } from "../settings/useVaultSettings.js";
import { useContextMenu } from "../menu/ContextMenu.js";
import type { ShortcutsState } from "../workspace/useShortcuts.js";

// Боковая панель целиком — шапка с поиском и «+», дерево, подвал
// (макет «Typsidian Доски» § Sidebar).
//
// Отдельный компонент, а не часть `Workspace`, по одной причине: состояние
// create/rename/delete нужно и кнопке в шапке, и строкам дерева, а кнопка по
// макету стоит **над** областью прокрутки. Держать это состояние в `Workspace`
// нельзя — там хук пришлось бы вызывать до того, как vault открыт, то есть без
// клиента. Здесь клиент есть по построению.

export interface SidebarProps {
  client: VaultClient;
  notes: Note[];
  /** Путь открытого файла — им отмечается активная строка. */
  openPath: string | null;
  folderCount: number;
  onOpenNote: (note: Note) => void;
  onNotesChanged: () => void | Promise<void>;
  onCreated: (path: string) => void;
  onError: (error: VaultOperationError) => void;
  onOpenGraph: () => void;
  /** Битых ссылок в vault (фича 028); ноль не показывается. */
  brokenLinks?: number;
  /** Открыть экран «Состояние ссылок». */
  onOpenLinkHealth?: (() => void) | undefined;
  /**
   * Заготовки для второго уровня меню «+» (фича 003, FR-020).
   *
   * Приходят сверху, а не читаются здесь: тот же состав нужен галерее, и второе
   * место, спрашивающее ядро о библиотеке, разошлось бы с первым — та же
   * причина, по которой разделы приходят пропом в дерево.
   */
  templates?: Template[];
  /** Открыть галерею заготовок целиком. */
  onOpenTemplates?: () => void;
  /**
   * Заготовка, выбранная в галерее для новой заметки (фича 003, FR-022).
   *
   * Приходит сюда, потому что имя и расположение задаются **существующими
   * средствами 001** — тем же инлайн-вводом в дереве, что и у пустой заметки.
   * Ревизия — чтобы повторный выбор той же заготовки снова открыл ввод.
   */
  createFromTemplate?: { template: Template; revision: number } | null;
  /** Действующие сочетания — уезжают в меню создания файла. */
  shortcuts: ShortcutsState;
  /** Вставить ссылку на заметку в открытую — пункт контекстного меню дерева. */
  onInsertLink?: ((note: Note) => void) | undefined;
  /** История файла из контекстного меню (фича 011, FR-009). */
  onFileHistory?: ((note: Note) => void) | undefined;
  /** Пути с неразрешённым конфликтом — дерево помечает их значком (фича 011). */
  conflictPaths?: ReadonlySet<string> | undefined;
  /**
   * Файлы перенесены в дереве (фича 023, FR-020): открытый файл и его
   * несохранённые правки следуют за путём. Пары приходят из ядра.
   */
  onPathsMoved?: ((moved: PathChange[]) => void) | undefined;
  /**
   * «Переместить в…» для открытого файла — из палитры команд (FR-021).
   * Ревизия — чтобы повторный вызов для того же файла снова открыл окно.
   */
  moveToRequest?: { entry: MoveEntryRef; revision: number } | null;
  /** Запрос принят — иначе панель, показанная заново, открыла бы окно ещё раз. */
  onMoveToRequestHandled?: (() => void) | undefined;
}

export function Sidebar({
  client,
  notes,
  openPath,
  folderCount,
  onOpenNote,
  onNotesChanged,
  onCreated,
  onError,
  onOpenGraph,
  brokenLinks = 0,
  onOpenLinkHealth,
  templates = [],
  onOpenTemplates,
  createFromTemplate = null,
  shortcuts,
  onInsertLink,
  onFileHistory,
  conflictPaths,
  onPathsMoved,
  moveToRequest = null,
  onMoveToRequestHandled,
}: SidebarProps): React.JSX.Element {
  const { t } = useI18n();
  const [filter, setFilter] = useState("");
  const contextMenu = useContextMenu();

  // Разделы читаются здесь, а не приходят сверху: дерево — единственное
  // место, где их помечают, а панель настроек пересоздаётся при каждом
  // открытии и читает актуальный набор сама (фича 006, FR-013, FR-014).
  const vaultSettings = useVaultSettings(client);
  const sections = useMemo(
    () => (vaultSettings.overrides?.sections ?? []).map((section) => section.path),
    [vaultSettings.overrides],
  );
  const toggleSection = useCallback(
    (folder: string, marked: boolean) =>
      marked
        ? vaultSettings.markSection(folder)
        : vaultSettings.unmarkSection(folder),
    [vaultSettings],
  );

  const actions = useVaultTreeActions({
    client,
    onNotesChanged,
    onCreated,
    onError,
    sections,
    onToggleSection: toggleSection,
  });

  // Перенос в дереве (фича 023): один путь для жеста, «Переместить в…» и
  // отмены. Что допустимо и что сломается, решает ядро.
  const move = useMoveEntry({
    client,
    onNotesChanged,
    onPathsMoved,
    onFoldersMoved: actions.remapExpanded,
  });
  const requestMove = move.requestMove;
  const showMoveNotice = move.showNotice;
  const [moveTo, setMoveTo] = useState<MoveEntryRef | null>(null);
  const requestedEntry = moveToRequest?.entry ?? null;
  const requestedRevision = moveToRequest?.revision ?? 0;
  useEffect(() => {
    if (requestedEntry === null) return;
    setMoveTo(requestedEntry);
    onMoveToRequestHandled?.();
    // Ревизия в зависимостях намеренно: она и есть повод открыть окно заново.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [requestedRevision]);

  const drag = useTreeDrag({
    disabled: actions.state.pendingPath !== null || move.busy,
    loadTargets: useCallback((from: string) => client.moveTargets(from, ""), [client]),
    // Недоступный на запись vault переноса не начинает и называет причину
    // (FR-006). Состояние доступа — у ядра, здесь оно только спрашивается.
    checkWritable: useCallback(
      async () => (await client.accessState()) === "readWrite",
      [client],
    ),
    onDenied: useCallback(
      () =>
        showMoveNotice({
          kind: "failed",
          message: (translate) => translate("tree.move.refused.accessDenied"),
        }),
      [showMoveNotice],
    ),
    onDrop: useCallback(
      (source, toFolder) => void requestMove({ path: source.path, name: source.name }, toFolder),
      [requestMove],
    ),
    onExpand: actions.expand,
  });

  // Выбор заготовки в галерее открывает тот же инлайн-ввод имени, что и «+»:
  // второго пути создания заметки не заводится (FR-022).
  const chosenTemplate = createFromTemplate?.template ?? null;
  const chosenRevision = createFromTemplate?.revision ?? 0;
  const beginCreate = actions.beginCreate;
  useEffect(() => {
    if (chosenTemplate === null) return;
    beginCreate("", "note", chosenTemplate);
  }, [chosenTemplate, chosenRevision, beginCreate]);

  // Поиск фильтрует то же дерево, а не заводит свой список.
  const visible = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (needle === "") return notes;
    return notes.filter((note) => note.name.toLowerCase().includes(needle));
  }, [notes, filter]);

  return (
    <aside className="ts-sidebar">
      {/* Шапка панели: поиск занимает всю ширину, «+» — квадрат 32px справа. */}
      <div className="ts-sidebar__search">
        <div className="ts-searchfield">
          <Icon name="search" size={13} />
          <input
            type="search"
            value={filter}
            aria-label={t("tree.search.label")}
            placeholder={t("tree.search.placeholder")}
            onChange={(event) => setFilter(event.target.value)}
          />
        </div>
        <CreateFileMenu
          folderName=""
          disabled={actions.state.pendingPath !== null}
          templates={templates}
          {...(onOpenTemplates ? { onOpenTemplates } : {})}
          onCreate={(kind: CreateKind, template?: Template | null) =>
            actions.beginCreate("", kind, template ?? null)
          }
          shortcuts={shortcuts}
        />
      </div>

      {/* Правый клик по пустому месту дерева — создание в корне. Строки
          заметок и папок открывают своё меню и до этого обработчика событие
          не пускают. */}
      <nav
        className="ts-tree"
        aria-label={t("tree.navLabel")}
        ref={drag.containerRef}
        // Пустое место дерева — бросок в корень vault (фича 023, FR-011).
        data-drop-folder=""
        onPointerDown={drag.onPointerDown}
        onContextMenu={(event) => {
          if (actions.state.pendingPath !== null) return;
          contextMenu.open(event, [
            { label: t("tree.menu.createNote"), run: () => actions.beginCreate("") },
            { label: t("tree.menu.createBoard"), run: () => actions.beginCreate("", "board") },
            ...(onOpenTemplates
              ? [{ label: t("tree.menu.fromTemplate"), run: onOpenTemplates }]
              : []),
          ]);
        }}
      >
        <VaultTreeActions
          notes={visible}
          openPath={openPath}
          actions={actions}
          onOpenNote={onOpenNote}
          onInsertLink={onInsertLink}
          onFileHistory={onFileHistory}
          conflictPaths={conflictPaths}
          onMoveTo={setMoveTo}
        />
      </nav>

      <TreeDragGhost
        source={drag.active}
        over={drag.over}
        ghostRef={drag.ghostRef}
        rootName={client.rootName}
      />
      <MoveNotice
        notice={move.notice}
        busy={move.busy}
        onUndo={() => void move.undo()}
        onDismiss={move.dismissNotice}
      />
      {moveTo !== null && (
        <MoveToDialog
          client={client}
          entry={moveTo}
          onPick={(folder) => void requestMove(moveTo, folder)}
          onClose={() => setMoveTo(null)}
        />
      )}
      <BacklinksConfirmDialog
        request={
          move.confirm === null
            ? null
            : {
                operation: "move",
                name: move.confirm.entry.name,
                folder: move.confirm.folder,
                affected: move.confirm.affected,
              }
        }
        onConfirm={() => move.answerConfirm(true)}
        onCancel={() => move.answerConfirm(false)}
      />

      <div className="ts-sidebar__footer">
        {/* Ход переноса большой папки — вместо счётчика, пока он идёт (макет
            «Большая папка едет»). На рабочем столе перенос атомарен и сюда не
            попадает. */}
        {move.progress !== null ? (
          <span className="ts-tree-progress" role="status">
            {t("tree.move.progress", move.progress)}
            <progress max={move.progress.total} value={move.progress.done} />
            <button type="button" className="ts-btn ts-btn--quiet" onClick={move.cancel}>
              {t("tree.move.cancel")}
            </button>
          </span>
        ) : (
        <span>
          {t("common.notesCount", { count: notes.length })} ·{" "}
          {t("tree.foldersCount", { count: folderCount })}
        </span>
        )}
        <span className="ts-spacer" />
        {/* Счётчик битых ссылок (фича 028, макет L3): ноль не показывается. */}
        {brokenLinks > 0 && onOpenLinkHealth !== undefined && (
          <button
            type="button"
            className="ts-tree-broken"
            title={t("linkHealth.footerTitle")}
            onClick={onOpenLinkHealth}
          >
            {t("linkHealth.footer", { count: brokenLinks })}
          </button>
        )}
        <button
          type="button"
          className="ts-btn ts-btn--ghost"
          title={t("tree.graph.title")}
          aria-label={t("tree.graph.open")}
          onClick={onOpenGraph}
        >
          <Icon name="network" />
        </button>
      </div>
    </aside>
  );
}
