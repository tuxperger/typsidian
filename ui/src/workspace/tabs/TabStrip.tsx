"use client";

import { memo, useEffect, useRef, useState } from "react";
import type { GroupSlot, TabId, TabView } from "../../wasm-core/typsidian_core.js";
import { useBookmarkActions } from "../../bookmarks/BookmarkActions.js";
import { Icon } from "../../icons/Icons.js";
import { useContextMenu, type ContextMenuItem } from "../../menu/ContextMenu.js";
import { useI18n } from "../../i18n/index.js";
import { hintWith, type ShortcutsState } from "../useShortcuts.js";

// Полоса вкладок группы (макет «Typsidian Вкладки» 1a, 1b). Слева «Назад» и
// «Вперёд» по истории группы, дальше вкладки, справа «разделить». Ничего не
// решает: каждое действие уходит в ядро через колбэк (принцип I).
//
// Порядок вкладок меняется указателем с захватом, как граница у `PaneDivider`:
// HTML5 drag-and-drop не работает на сенсорном экране WebView и не даёт
// управлять видом переносимого (research.md §13). С клавиатуры — палитра и
// `Mod+Alt+Shift+\`.

/** Сколько пикселей указатель должен пройти, чтобы нажатие стало перетаскиванием. */
const DRAG_THRESHOLD_PX = 4;

export interface TabStripProps {
  slot: GroupSlot;
  tabs: readonly TabView[];
  activeId: TabId | undefined;
  /** Группа с кареткой: линия активной вкладки оранжевая, у другой — серая (макет 1b). */
  focused: boolean;
  canBack: boolean;
  canForward: boolean;
  /** Есть вторая группа: кнопка читается как «убрать разделение». */
  split: boolean;
  /** Кнопка разделения в этой полосе (макет: у последней полосы). */
  showSplitControl: boolean;
  canSplit: boolean;
  canUnsplit: boolean;
  canMoveToOther: boolean;
  shortcuts: ShortcutsState;
  onActivate(tab: TabId): void;
  onClose(tab: TabId): void;
  onCloseOthers(tab: TabId): void;
  onBack(): void;
  onForward(): void;
  onCycle(direction: "Next" | "Previous"): void;
  /** Разделить или убрать разделение; для вкладки, на которой вызвано. */
  onToggleSplit(tab: TabId): void;
  onMoveToOther(tab: TabId): void;
  /** Перетаскивание: вкладка → группа и место в ней (место считается без самой вкладки). */
  onMove(tab: TabId, group: GroupSlot, index: number): void;
}

interface DragState {
  tab: TabId;
  /** Куда упадёт вкладка: группа и вкладка, перед которой она встанет (`null` — в конец). */
  group: GroupSlot;
  before: TabId | null;
  /** Место среди вкладок полосы под указателем, без самой переносимой. */
  index: number;
}

/**
 * Полоса вкладок группы. `memo`: рабочая область перерисовывается на каждый
 * набранный знак, полосе это не нужно — её пропы при наборе не меняются
 * (колбэки постоянны, см. `EditorGroup`; FR-027, T070).
 */
export const TabStrip = memo(TabStripView);

function TabStripView(props: TabStripProps): React.JSX.Element {
  const {
    slot,
    tabs,
    activeId,
    focused,
    canBack,
    canForward,
    split,
    showSplitControl,
    canSplit,
    canUnsplit,
    canMoveToOther,
    shortcuts,
  } = props;
  const { t } = useI18n();
  const contextMenu = useContextMenu();
  const stripRef = useRef<HTMLDivElement | null>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  // Начатое нажатие: перетаскиванием оно становится после порога.
  const press = useRef<{ tab: TabId; x: number; y: number; pointer: number; dragging: boolean } | null>(
    null,
  );
  const suppressClick = useRef(false);

  // Активная вкладка всегда видна (FR-008). Прокрутка считается вручную: у
  // `scrollIntoView` побочный эффект на всех предков, а полосе нужна только своя.
  // Положение берётся из прямоугольников: `offsetLeft` отсчитан не от полосы.
  // Полоса сужается и без смены вкладки (разделение, граница групп, окно).
  useEffect(() => {
    const strip = stripRef.current;
    if (strip === null || activeId === undefined) return;
    const reveal = (): void => {
      const element = strip.querySelector<HTMLElement>(`[data-tab-id="${activeId}"]`);
      if (element === null) return;
      const bounds = strip.getBoundingClientRect();
      const rect = element.getBoundingClientRect();
      if (rect.left < bounds.left) strip.scrollLeft -= bounds.left - rect.left;
      else if (rect.right > bounds.right) strip.scrollLeft += rect.right - bounds.right;
    };
    reveal();
    const observer = new ResizeObserver(reveal);
    observer.observe(strip);
    return () => observer.disconnect();
  }, [activeId, tabs.length]);

  /** Подпись сочетания пункта меню; снятое сочетание подписи не оставляет. */
  const keyed = (command: Parameters<ShortcutsState["displayOf"]>[0]): { shortcut?: string } => {
    const value = shortcuts.displayOf(command);
    return value === "" ? {} : { shortcut: value };
  };

  // «В закладки» (фича 032) — из контекста рабочей области.
  const bookmarkActions = useBookmarkActions();
  const openMenu = (event: React.MouseEvent, tab: TabView): void => {
    const items: ContextMenuItem[] = [
      {
        label: t("workspace.tabs.close"),
        ...keyed("tab-close"),
        run: () => props.onClose(tab.id),
      },
      {
        label: t("workspace.tabs.closeOthers"),
        disabled: tabs.length < 2,
        run: () => props.onCloseOthers(tab.id),
      },
      { kind: "separator" },
      {
        label: split ? t("workspace.tabs.unsplit") : t("workspace.tabs.split"),
        ...keyed("split-editor"),
        disabled: split ? !canUnsplit : !canSplit,
        run: () => props.onToggleSplit(tab.id),
      },
      {
        label: t("workspace.tabs.moveToOther"),
        ...keyed("tab-move-to-other-group"),
        disabled: !canMoveToOther,
        run: () => props.onMoveToOther(tab.id),
      },
      { kind: "separator" },
      {
        label: t("workspace.tabs.next"),
        ...keyed("tab-next"),
        run: () => props.onCycle("Next"),
      },
      {
        label: t("workspace.tabs.previous"),
        ...keyed("tab-previous"),
        run: () => props.onCycle("Previous"),
      },
    ];
    if (bookmarkActions !== null) {
      items.push(
        { kind: "separator" },
        {
          label: t("bookmarks.add"),
          run: () => bookmarkActions.add({ kind: "Note", path: tab.path }, tab.title),
        },
      );
    }
    contextMenu.open(event, items);
  };

  // ── Перетаскивание ────────────────────────────────────────────────────

  /** Куда упадёт вкладка под указателем: полоса под ним и вкладка, перед которой встанет. */
  const dropTargetAt = (x: number, y: number, dragged: TabId): DragState | null => {
    const strips = document
      .elementsFromPoint(x, y)
      .filter((element): element is HTMLElement => element instanceof HTMLElement)
      .find((element) => element.dataset["stripGroup"] !== undefined);
    if (strips === undefined) return null;
    const group = strips.dataset["stripGroup"] === "Second" ? "Second" : "First";
    const candidates = [...strips.querySelectorAll<HTMLElement>("[data-tab-id]")].filter(
      (element) => Number(element.dataset["tabId"]) !== dragged,
    );
    const before = candidates.find((element) => {
      const rect = element.getBoundingClientRect();
      return x < rect.left + rect.width / 2;
    });
    return {
      tab: dragged,
      group,
      before: before === undefined ? null : Number(before.dataset["tabId"]),
      index: before === undefined ? candidates.length : candidates.indexOf(before),
    };
  };

  // Чужая полоса о перетаскивании не знает (состояние — у полосы-источника):
  // место падения на ней отмечается атрибутом, а не её состоянием.
  const markDropStrip = (target: DragState | null): void => {
    for (const strip of document.querySelectorAll<HTMLElement>("[data-strip-group]")) {
      if (target !== null && target.group !== slot && strip.dataset["stripGroup"] === target.group) {
        strip.dataset["drop"] = "true";
      } else {
        delete strip.dataset["drop"];
      }
    }
  };

  const finishDrag = (target: DragState | null): void => {
    setDrag(null);
    markDropStrip(null);
    if (target === null) return;
    // Индекс — среди вкладок целевой группы без самой переносимой. Полоса под
    // указателем может быть чужой: её вкладок в `tabs` нет, место взято из неё.
    if (!showAllGroups(tabs)) {
      props.onMove(target.tab, target.group, target.index);
      return;
    }
    // В слитой полосе вкладки двух групп стоят вперемешку, поэтому вкладка
    // остаётся в своей группе и считаются только свои.
    const group = tabs.find((candidate) => candidate.id === target.tab)?.group ?? target.group;
    const peers = tabs.filter((candidate) => candidate.group === group && candidate.id !== target.tab);
    let index = peers.length;
    if (target.before !== null) {
      const at = peers.findIndex((candidate) => candidate.id === target.before);
      if (at >= 0) index = at;
    }
    props.onMove(target.tab, group, index);
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
    const current = (event.target as HTMLElement).closest<HTMLElement>("[data-tab-id]");
    if (current === null) return;
    const id = Number(current.dataset["tabId"]);
    const elements = [...event.currentTarget.querySelectorAll<HTMLElement>("[data-tab-id]")];
    const at = elements.indexOf(current);
    const focusAt = (index: number): void => {
      const target = elements[(index + elements.length) % elements.length];
      target?.focus();
    };
    switch (event.key) {
      case "ArrowRight":
        event.preventDefault();
        focusAt(at + 1);
        break;
      case "ArrowLeft":
        event.preventDefault();
        focusAt(at - 1);
        break;
      case "Home":
        event.preventDefault();
        focusAt(0);
        break;
      case "End":
        event.preventDefault();
        focusAt(elements.length - 1);
        break;
      case "Enter":
      case " ":
        event.preventDefault();
        props.onActivate(id);
        break;
      case "Delete":
        event.preventDefault();
        props.onClose(id);
        break;
      default:
        break;
    }
  };

  const rovingId = activeId ?? tabs[0]?.id;

  return (
    <div className="ts-tabs" data-focused={focused ? "true" : "false"} data-slot={slot}>
      <div className="ts-tabs__nav" role="group" aria-label={t("workspace.tabs.groupLabel", { group: slot === "First" ? 1 : 2 })}>
        <button
          type="button"
          className="ts-tabs__navbtn"
          disabled={!canBack}
          title={hintWith(t("workspace.tabs.back"), shortcuts.displayOf("navigate-back"))}
          aria-label={t("workspace.tabs.back")}
          onClick={props.onBack}
        >
          <Icon name="chevron" size={12} className="ts-icon--flip" />
        </button>
        <button
          type="button"
          className="ts-tabs__navbtn"
          disabled={!canForward}
          title={hintWith(t("workspace.tabs.forward"), shortcuts.displayOf("navigate-forward"))}
          aria-label={t("workspace.tabs.forward")}
          onClick={props.onForward}
        >
          <Icon name="chevron" size={12} />
        </button>
      </div>

      <div
        ref={stripRef}
        className="ts-tabs__strip"
        role="tablist"
        aria-label={t("workspace.tabs.label")}
        data-strip-group={slot}
        onKeyDown={onKeyDown}
      >
        {tabs.map((tab) => {
          const selected = tab.id === activeId;
          const classes = ["ts-tab"];
          if (selected) classes.push("ts-tab--on");
          if (drag?.tab === tab.id) classes.push("ts-tab--dragging");
          if (drag !== null && drag.before === tab.id) classes.push("ts-tab--drop-before");
          return (
            <div
              key={tab.id}
              className={classes.join(" ")}
              role="tab"
              aria-selected={selected}
              tabIndex={tab.id === rovingId ? 0 : -1}
              data-tab-id={tab.id}
              data-kind={tab.kind}
              onClick={() => {
                if (suppressClick.current) {
                  suppressClick.current = false;
                  return;
                }
                props.onActivate(tab.id);
              }}
              onAuxClick={(event) => {
                // Средняя кнопка закрывает, как в браузере.
                if (event.button === 1) {
                  event.preventDefault();
                  props.onClose(tab.id);
                }
              }}
              onContextMenu={(event) => openMenu(event, tab)}
              onPointerDown={(event) => {
                if (event.button !== 0 || (event.target as HTMLElement).closest(".ts-tab__close")) return;
                press.current = {
                  tab: tab.id,
                  x: event.clientX,
                  y: event.clientY,
                  pointer: event.pointerId,
                  dragging: false,
                };
              }}
              onPointerMove={(event) => {
                const started = press.current;
                if (started === null || started.tab !== tab.id) return;
                if (!started.dragging) {
                  const far =
                    Math.abs(event.clientX - started.x) > DRAG_THRESHOLD_PX ||
                    Math.abs(event.clientY - started.y) > DRAG_THRESHOLD_PX;
                  if (!far) return;
                  started.dragging = true;
                  // Захват только теперь: раньше он забрал бы щелчок у кнопки ×.
                  event.currentTarget.setPointerCapture(started.pointer);
                }
                const target = dropTargetAt(event.clientX, event.clientY, tab.id);
                setDrag(target);
                markDropStrip(target);
              }}
              onPointerUp={(event) => {
                const started = press.current;
                press.current = null;
                if (started === null || !started.dragging) return;
                event.currentTarget.releasePointerCapture(started.pointer);
                suppressClick.current = true;
                window.setTimeout(() => {
                  suppressClick.current = false;
                }, 0);
                finishDrag(dropTargetAt(event.clientX, event.clientY, tab.id));
              }}
              onPointerCancel={() => {
                press.current = null;
                setDrag(null);
                markDropStrip(null);
              }}
            >
              <span className="ts-tab__title">{tab.title}</span>
              {tab.hint !== undefined && <span className="ts-tab__hint">{tab.hint}</span>}
              {tab.unsaved && (
                <span
                  className="ts-tab__dot"
                  role="img"
                  aria-label={t("workspace.tabs.unsaved")}
                  title={t("workspace.tabs.unsaved")}
                />
              )}
              {tab.conflict && (
                <span className="ts-tab__tag ts-tab__tag--warn">{t("workspace.tabs.conflict")}</span>
              )}
              {tab.kind === "Board" && <span className="ts-tab__tag">{t("workspace.tabs.board")}</span>}
              <button
                type="button"
                className="ts-tab__close"
                tabIndex={-1}
                aria-label={t("workspace.tabs.closeNamed", { title: tab.title })}
                title={hintWith(t("workspace.tabs.close"), shortcuts.displayOf("tab-close"))}
                onClick={(event) => {
                  event.stopPropagation();
                  props.onClose(tab.id);
                }}
              >
                <Icon name="x" size={10} />
              </button>
            </div>
          );
        })}
        {drag !== null && drag.before === null && drag.group === slot && (
          <span className="ts-tabs__drop-end" aria-hidden="true" />
        )}
      </div>

      {showSplitControl && (
        <div className="ts-tabs__nav ts-tabs__nav--end">
          <button
            type="button"
            className="ts-tabs__navbtn"
            aria-pressed={split}
            disabled={split ? !canUnsplit : !canSplit}
            title={hintWith(
              split ? t("workspace.tabs.unsplit") : t("workspace.tabs.split"),
              shortcuts.displayOf("split-editor"),
            )}
            aria-label={split ? t("workspace.tabs.unsplit") : t("workspace.tabs.split")}
            onClick={() => {
              const target = activeId ?? tabs[0]?.id;
              if (target !== undefined) props.onToggleSplit(target);
            }}
          >
            <Icon name="layout" size={12} />
          </button>
        </div>
      )}
    </div>
  );
}

/** В слитой полосе вкладки двух групп стоят вперемешку. */
function showAllGroups(tabs: readonly TabView[]): boolean {
  return tabs.some((tab) => tab.group === "First") && tabs.some((tab) => tab.group === "Second");
}
