"use client";

import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import type { WorkspaceView } from "../../wasm-core/typsidian_core.js";
import type { VaultClient } from "../../vault-client.js";
import type { BoardControllers } from "../../boards/boardControllers.js";
import type { NoteDocumentRegistry } from "../../editor/noteDocuments.js";
import { useI18n } from "../../i18n/index.js";
import { PaneDivider } from "../PaneDivider.js";
import type { ShortcutsState } from "../useShortcuts.js";
import { EditorGroup, type SharedEditorProps } from "./EditorGroup.js";
import type { WorkspaceTabs } from "./useWorkspaceTabs.js";

// Одна или две группы вкладок и граница между ними (макет «Typsidian Вкладки»
// 1a, 1b). Сколько групп показать — решает ядро по ширине окна (`presentation`);
// здесь только раскладка и перетаскивание границы.

/** Шаг клавиатуры, промилле. Ядро подрежет доли, выходящие за допустимые. */
const RATIO_STEP = 20;
/** Доля по умолчанию — поровну; совпадает с `WorkspaceState::empty`. */
const RATIO_DEFAULT = 500;
/** Пауза перед отправкой доли в ядро: перетаскивание меняет её каждый кадр. */
const RATIO_COMMIT_MS = 120;

export interface EditorGroupsProps {
  view: WorkspaceView;
  tabs: WorkspaceTabs;
  client: VaultClient;
  registry: NoteDocumentRegistry;
  boardControllers: BoardControllers;
  shortcuts: ShortcutsState;
  editor: SharedEditorProps;
  linksSummary: string | undefined;
  /** Класс контейнера: без колонки предпросмотра группы занимают всё место. */
  solo: boolean;
}

export function EditorGroups(props: EditorGroupsProps): React.JSX.Element {
  const { view, tabs } = props;
  const { t } = useI18n();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const split = view.groups.length === 2;
  const list = view.presentation === "List";

  // Доля показывается по ходу перетаскивания сразу, а в ядро уходит с задержкой.
  const [liveRatio, setLiveRatio] = useState<number | null>(null);
  const commitTimer = useRef<number | null>(null);
  const ratio = liveRatio ?? view.ratio_permille;
  useEffect(() => setLiveRatio(null), [view.ratio_permille]);
  useEffect(
    () => () => {
      if (commitTimer.current !== null) window.clearTimeout(commitTimer.current);
    },
    [],
  );

  const commitRatio = useCallback(
    (permille: number) => {
      const rounded = Math.round(permille);
      setLiveRatio(rounded);
      if (commitTimer.current !== null) window.clearTimeout(commitTimer.current);
      commitTimer.current = window.setTimeout(() => {
        commitTimer.current = null;
        void tabs.dispatch({ kind: "SetRatio", permille: rounded });
      }, RATIO_COMMIT_MS);
    },
    [tabs],
  );

  const activeSlot = view.active_group;
  const summary = split
    ? t("workspace.tabs.groupOf", {
        group: activeSlot === "First" ? 1 : 2,
        count: view.tab_count,
      })
    : t("workspace.tabs.count", { count: view.tab_count });

  // Разделение и перенос действуют на активную вкладку: правым щелчком по
  // другой вкладке сначала делаем её активной.
  const withTab = useCallback(
    (tab: number, action: () => Promise<unknown>) => {
      void (async () => {
        if (view.active?.tab !== tab) await tabs.dispatch({ kind: "Activate", tab });
        await action();
      })();
    },
    [view.active?.tab, tabs],
  );

  const toggleSplit = useCallback(
    (tab: number) =>
      withTab(tab, () =>
        tabs.dispatch(split ? { kind: "Unsplit" } : { kind: "Split" }),
      ),
    [withTab, tabs, split],
  );
  const moveToOther = useCallback(
    (tab: number) =>
      withTab(tab, () => tabs.dispatch({ kind: "MoveToOtherGroup", tab })),
    [withTab, tabs],
  );

  const classes = ["ts-groups"];
  if (props.solo) classes.push("ts-groups--solo");
  if (split) classes.push("ts-groups--split");

  return (
    <div className={classes.join(" ")} ref={containerRef}>
      {view.groups.map((group, index) => (
        <Fragment key={group.slot}>
          {index === 1 && (
            <PaneDivider
              label={t("workspace.tabs.groupDivider")}
              now={ratio / 10}
              min={0}
              max={100}
              onDrag={(clientX) => {
                const rect = containerRef.current?.getBoundingClientRect();
                if (rect === undefined || rect.width <= 0) return;
                commitRatio(((clientX - rect.left) / rect.width) * 1000);
              }}
              onStep={(direction) => commitRatio(ratio + direction * RATIO_STEP)}
              onReset={() => commitRatio(RATIO_DEFAULT)}
            />
          )}
          <div
            className="ts-groups__cell"
            style={split ? { flexGrow: index === 0 ? ratio : 1000 - ratio } : undefined}
          >
            <EditorGroup
              group={group}
              view={view}
              focused={group.slot === activeSlot}
              split={split}
              hideStrip={list}
              showSplitControl={index === view.groups.length - 1}
              client={props.client}
              registry={props.registry}
              boardControllers={props.boardControllers}
              shortcuts={props.shortcuts}
              tabs={tabs}
              editor={props.editor}
              linksSummary={props.linksSummary}
              tabsSummary={summary}
              onToggleSplit={toggleSplit}
              onMoveToOther={moveToOther}
            />
          </div>
        </Fragment>
      ))}
    </div>
  );
}
