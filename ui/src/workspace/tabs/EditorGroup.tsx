"use client";

import type { GroupView, TabView, WorkspaceView } from "../../wasm-core/typsidian_core.js";
import type { VaultClient } from "../../vault-client.js";
import { BoardTab } from "../../boards/BoardTab.js";
import type { BoardControllers } from "../../boards/boardControllers.js";
import { NoteEditor, type NoteEditorProps } from "../../editor/NoteEditor.js";
import type { NoteDocumentRegistry } from "../../editor/noteDocuments.js";
import { useNoteDocument } from "../../editor/useNoteDocument.js";
import { useI18n } from "../../i18n/index.js";
import { basename } from "../../paths.js";
import type { ShortcutsState } from "../useShortcuts.js";
import { CloseRefusedNotice } from "./CloseRefusedNotice.js";
import { TabStrip } from "./TabStrip.js";
import type { WorkspaceTabs } from "./useWorkspaceTabs.js";

// Группа вкладок: полоса и содержимое активной вкладки — заметка, доска либо
// пустое состояние (макет «Typsidian Вкладки» 1a, 1c).

/** Что у всех редакторов заметок общее: приходит из рабочей области. */
export type SharedEditorProps = Pick<
  NoteEditorProps,
  | "boards"
  | "externalChange"
  | "onOpenBoard"
  | "onCreateBoard"
  | "onEditorReady"
  | "onSaveAsTemplate"
  | "fontSize"
  | "shortcuts"
  | "paramsRequest"
>;

export interface EditorGroupProps {
  group: GroupView;
  view: WorkspaceView;
  /** В группе каретка: за ней следуют предпросмотр и колонка заметки. */
  focused: boolean;
  /** Показано две группы. */
  split: boolean;
  /** Полоса не нужна — телефон, `List`. */
  hideStrip: boolean;
  /** Кнопка разделения стоит в полосе этой группы. */
  showSplitControl: boolean;
  client: VaultClient;
  registry: NoteDocumentRegistry;
  boardControllers: BoardControllers;
  shortcuts: ShortcutsState;
  tabs: WorkspaceTabs;
  editor: SharedEditorProps;
  /** Связи и сводка вкладок — только редактору группы с кареткой. */
  linksSummary: string | undefined;
  tabsSummary: string | undefined;
  onToggleSplit(tab: number): void;
  onMoveToOther(tab: number): void;
}

export function EditorGroup(props: EditorGroupProps): React.JSX.Element {
  const { group, view, focused, split, hideStrip, tabs, shortcuts } = props;
  const { t } = useI18n();
  const activeTab = group.tabs.find((tab) => tab.id === group.active);

  return (
    <section
      className="ts-egroup"
      data-focused={focused ? "true" : "false"}
      data-slot={group.slot}
      aria-label={t("workspace.tabs.groupLabel", { group: group.slot === "First" ? 1 : 2 })}
      // Каретка перешла в эту группу (US3 AS4): модель узнаёт об этом по фокусу.
      onFocusCapture={() => {
        if (split && !focused) void tabs.dispatch({ kind: "FocusGroup", group: group.slot });
      }}
      onPointerDownCapture={() => {
        if (split && !focused) void tabs.dispatch({ kind: "FocusGroup", group: group.slot });
      }}
    >
      {!hideStrip && (
        <TabStrip
          slot={group.slot}
          tabs={group.tabs}
          activeId={group.active}
          focused={focused}
          canBack={group.can_back}
          canForward={group.can_forward}
          split={split}
          showSplitControl={props.showSplitControl}
          canSplit={view.can_split}
          canUnsplit={view.can_unsplit}
          canMoveToOther={view.can_move_to_other}
          shortcuts={shortcuts}
          onActivate={(tab) => void tabs.activate(tab)}
          onClose={(tab) => void tabs.close(tab)}
          onCloseOthers={(tab) => void tabs.closeOthers(tab)}
          onBack={() => void tabs.dispatch({ kind: "Back" })}
          onForward={() => void tabs.dispatch({ kind: "Forward" })}
          onCycle={(direction) => void tabs.dispatch({ kind: "Cycle", direction })}
          onToggleSplit={props.onToggleSplit}
          onMoveToOther={props.onMoveToOther}
          onMove={(tab, slot, index) =>
            void tabs.dispatch({ kind: "Move", tab, group: slot, index })
          }
        />
      )}

      <div className="ts-egroup__body">
        {activeTab === undefined ? (
          <EmptyGroup shortcuts={shortcuts} />
        ) : activeTab.kind === "Board" ? (
          <BoardTab
            key={activeTab.id}
            client={props.client}
            path={activeTab.path}
            name={basename(activeTab.path)}
            controllers={props.boardControllers}
          />
        ) : (
          <NoteTab
            key={activeTab.id}
            tab={activeTab}
            focused={focused}
            props={props}
          />
        )}
      </div>

      {focused && tabs.refusal !== null && (
        <CloseRefusedNotice refusal={tabs.refusal} onDismiss={tabs.dismissRefusal} />
      )}
    </section>
  );
}

/** Заметка вкладки: документ из реестра и редактор над ним. */
function NoteTab({
  tab,
  focused,
  props,
}: {
  tab: TabView;
  focused: boolean;
  props: EditorGroupProps;
}): React.JSX.Element {
  const { t } = useI18n();
  const document = useNoteDocument(props.registry, tab.path);
  const { tabs } = props;
  const place = tabs.pendingPlace?.tab === tab.id ? tabs.pendingPlace.place : null;

  if (document === null) {
    return (
      <section className="ts-pane ts-pane--editor" aria-label={t("editor.label")}>
        <div className="ts-placeholder">
          <p role="status">{t("common.loading")}</p>
        </div>
      </section>
    );
  }
  return (
    <NoteEditor
      client={props.client}
      typst={tab.kind === "Note"}
      document={document}
      name={basename(tab.path)}
      tabId={tab.id}
      active={focused}
      place={place}
      onPlaceShown={tabs.placeShown}
      focusRequest={tabs.focusRequest}
      {...props.editor}
      {...(focused && props.linksSummary !== undefined ? { linksSummary: props.linksSummary } : {})}
      {...(focused && props.tabsSummary !== undefined ? { tabsSummary: props.tabsSummary } : {})}
    />
  );
}

/** Пустое состояние (макет 1c): заметка не открыта, подсказка про палитру. */
function EmptyGroup({ shortcuts }: { shortcuts: ShortcutsState }): React.JSX.Element {
  const { t } = useI18n();
  const key = shortcuts.displayOf("command-palette");
  return (
    <div className="ts-placeholder ts-egroup__empty">
      <p role="status">
        {t("workspace.tabs.empty")}
        <br />
        {t("workspace.tabs.emptyHint")}
        {key !== "" && <> <span className="ts-tab__tag">{key}</span></>}
      </p>
    </div>
  );
}
