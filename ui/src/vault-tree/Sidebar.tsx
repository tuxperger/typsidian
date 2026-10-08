"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import type { VaultClient } from "../vault-client.js";
import type { Note, Template } from "../wasm-core/typsidian_core.js";
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
  templates = [],
  onOpenTemplates,
  createFromTemplate = null,
  shortcuts,
  onInsertLink,
  onFileHistory,
  conflictPaths,
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
        />
      </nav>

      <div className="ts-sidebar__footer">
        <span>
          {t("common.notesCount", { count: notes.length })} ·{" "}
          {t("tree.foldersCount", { count: folderCount })}
        </span>
        <span className="ts-spacer" />
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
