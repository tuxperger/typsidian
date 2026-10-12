"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type {
  GroupSlot,
  RefusalReason,
  TabId,
  TabView,
  TextPlace,
  Transition,
  VaultEvent,
  VaultId,
  WorkspaceAction,
  WorkspaceState,
  WorkspaceView,
} from "../../wasm-core/typsidian_core.js";
import type { VaultClient } from "../../vault-client.js";
import type { Phrase } from "../../i18n/index.js";
import type { BoardControllers } from "../../boards/boardControllers.js";
import type { NoteDocumentRegistry } from "../../editor/noteDocuments.js";
import { tabEditorStates } from "../../editor/tabEditorStates.js";
import type { WorkspaceTabsStore } from "./tabs-store.js";

// Вкладки в интерфейсе (фича 014, contracts/frontend-workspace.md).
//
// Хук ничего не решает о вкладках: он отправляет действие в ядро, получает
// `Transition`, исполняет `focus`/`closed`/`refused` и хранит последний
// `WorkspaceView` для показа (принцип I). Значение состояния живёт здесь, в ref:
// ядро чистое и состояния не держит, а действия исполняются строго по очереди —
// следующее уходит с тем состоянием, которое вернуло предыдущее.
//
// Набор текста сюда не доходит вовсе (FR-027): перечитывание показа идёт после
// каждого перехода, по событиям vault о путях открытых вкладок, при переходе
// документа «чисто ↔ несохранённое» и когда ширина окна устоялась.

/** Пауза перед записью набора на устройство. */
export const TABS_PERSIST_DEBOUNCE_MS = 400;
/** Пауза после последнего изменения ширины окна, прежде чем перечитать показ. */
const WIDTH_SETTLE_MS = 150;
/** Сколько недавних вкладок держат документ загруженным (research.md §4). */
const PINNED_DOCUMENTS = 20;
/** Пауза склейки запросов на перечитывание показа. */
const REFRESH_COALESCE_MS = 80;

type WithoutLeaving<A> = A extends { leaving?: TextPlace } ? Omit<A, "leaving"> : A;

/** Действие без места ухода: его подставляет хук из `tabEditorStates`. */
export type WorkspaceActionInput = WithoutLeaving<WorkspaceAction>;

/** Отказ закрыть вкладку — для панели «Вкладка не закрыта». */
export interface TabRefusal {
  tab: TabId;
  path: string;
  title: string;
  reason: RefusalReason;
  /** Причина отказа записи словами документа; `null`, когда её нет. */
  detail: Phrase | null;
}

/** Место, куда надо поставить каретку после перехода. */
export interface PendingPlace {
  tab: TabId;
  place: TextPlace;
}

export interface WorkspaceTabs {
  /** `null` — vault не открыт или набор ещё читается. */
  view: WorkspaceView | null;
  dispatch(action: WorkspaceActionInput): Promise<Transition | null>;
  /** Открыть заметку или доску: дерево, поиск, палитра, граф, ссылка, переход к месту. */
  open(path: string, target?: TextPlace): Promise<void>;
  /** Дописать документ → `Close` → исполнить отказ. */
  close(tab: TabId): Promise<void>;
  /** Закрыть остальные вкладки группы вкладки. */
  closeOthers(tab: TabId): Promise<void>;
  /** Сдвинуть вкладку по полосе своей группы. */
  moveBy(tab: TabId, delta: -1 | 1): Promise<void>;
  /** Сделать вкладку активной и отдать фокус редактору. */
  activate(tab: TabId): Promise<void>;
  refusal: TabRefusal | null;
  dismissRefusal(): void;
  /** Место, куда редактор активной вкладки должен встать. */
  pendingPlace: PendingPlace | null;
  placeShown(): void;
  /** Растёт, когда фокус надо отдать редактору. */
  focusRequest: number;
  /** Событие vault: переименование, удаление, внешнее изменение. */
  noticeEvent(event: VaultEvent): void;
  /** Перечитать показ (признаки несохранённого и конфликта). */
  refresh(): void;
}

export interface WorkspaceTabsOptions {
  client: VaultClient | null;
  registry: NoteDocumentRegistry | null;
  boards: BoardControllers;
  store?: WorkspaceTabsStore | undefined;
  /** Ширина окна: подачу выбирает ядро. */
  widthPx: number;
}

/** Действия, после которых пользователь уходит с активной вкладки. */
const LEAVING_KINDS: ReadonlySet<WorkspaceAction["kind"]> = new Set([
  "Open",
  "Activate",
  "Cycle",
  "Back",
  "Forward",
  "Split",
]);

function attachLeaving(action: WorkspaceActionInput, leaving: TextPlace | null): WorkspaceAction {
  if (leaving === null) return action;
  switch (action.kind) {
    case "Open":
    case "Activate":
    case "Cycle":
    case "Split":
    case "FocusGroup":
    case "Back":
    case "Forward":
      return { ...action, leaving };
    default:
      return action;
  }
}

function allTabs(view: WorkspaceView | null): TabView[] {
  return view === null ? [] : view.groups.flatMap((group) => group.tabs);
}

function findTab(view: WorkspaceView | null, tab: TabId): TabView | undefined {
  return allTabs(view).find((entry) => entry.id === tab);
}

export function useWorkspaceTabs({
  client,
  registry,
  boards,
  store,
  widthPx,
}: WorkspaceTabsOptions): WorkspaceTabs {
  const [view, setView] = useState<WorkspaceView | null>(null);
  const [refusal, setRefusal] = useState<TabRefusal | null>(null);
  const [pendingPlace, setPendingPlace] = useState<PendingPlace | null>(null);
  const [focusRequest, setFocusRequest] = useState(0);

  const stateRef = useRef<WorkspaceState | null>(null);
  const viewRef = useRef<WorkspaceView | null>(null);
  const vaultRef = useRef<VaultId | null>(null);
  const widthRef = useRef(widthPx);
  widthRef.current = widthPx;
  const queueRef = useRef<Promise<unknown>>(Promise.resolve());
  /** Поколение: смена клиента обесценивает всё, что ещё в пути. */
  const generation = useRef(0);
  const recentPaths = useRef<string[]>([]);
  const persistTimer = useRef<number | null>(null);
  const refreshTimer = useRef<number | null>(null);

  // Клиент, реестр и порты читаются из ref: колбэки наружу обязаны быть
  // стабильными — они уезжают в подписки на события.
  const clientRef = useRef(client);
  clientRef.current = client;
  const registryRef = useRef(registry);
  registryRef.current = registry;
  const storeRef = useRef(store);
  storeRef.current = store;

  const enqueue = useCallback(<T,>(job: () => Promise<T>): Promise<T> => {
    const run = queueRef.current.then(job, job);
    queueRef.current = run.catch(() => undefined);
    return run;
  }, []);

  // ── Сохранение набора (FR-021) ────────────────────────────────────────

  const persistNow = useCallback(async (via?: VaultClient): Promise<void> => {
    if (persistTimer.current !== null) {
      window.clearTimeout(persistTimer.current);
      persistTimer.current = null;
    }
    // Снимок — чистая функция ядра, клиент нужен только как канал; при смене
    // vault «текущий» уже новый, поэтому уходящий передаётся явно.
    const current = via ?? clientRef.current;
    const target = storeRef.current;
    const state = stateRef.current;
    const vault = vaultRef.current;
    if (current === null || target === undefined || state === null || vault === null) return;
    try {
      const saved = await current.workspaceSnapshot(state);
      await target.save(vault, saved);
    } catch {
      // Отказ хранилища не ломает вкладки: потеряется только память о них (FR-023).
    }
  }, []);

  const schedulePersist = useCallback(() => {
    if (storeRef.current === undefined || vaultRef.current === null) return;
    if (persistTimer.current !== null) window.clearTimeout(persistTimer.current);
    persistTimer.current = window.setTimeout(() => void persistNow(), TABS_PERSIST_DEBOUNCE_MS);
  }, [persistNow]);

  // ── Показ ─────────────────────────────────────────────────────────────

  /** Принять новый показ: состояние React и закрепление документов недавних вкладок. */
  const publishView = useCallback((next: WorkspaceView) => {
    viewRef.current = next;
    setView(next);
    const open = new Set(allTabs(next).map((tab) => tab.path));
    const active = next.active?.path;
    const recent = recentPaths.current.filter((path) => open.has(path) && path !== active);
    if (active !== undefined) recent.unshift(active);
    for (const path of open) if (!recent.includes(path)) recent.push(path);
    recentPaths.current = recent;
    registryRef.current?.pin(recent.slice(0, PINNED_DOCUMENTS));
  }, []);

  const readView = useCallback(
    async (gen: number): Promise<void> => {
      const current = clientRef.current;
      const state = stateRef.current;
      if (current === null || state === null || gen !== generation.current) return;
      const next = await current.workspaceView(state, widthRef.current);
      if (gen === generation.current) publishView(next);
    },
    [publishView],
  );

  const refresh = useCallback(() => {
    if (refreshTimer.current !== null) return;
    refreshTimer.current = window.setTimeout(() => {
      refreshTimer.current = null;
      const gen = generation.current;
      void enqueue(() => readView(gen)).catch(() => undefined);
    }, REFRESH_COALESCE_MS);
  }, [enqueue, readView]);

  // ── Переходы ──────────────────────────────────────────────────────────

  const caretOfActive = useCallback((state: WorkspaceState): TextPlace | null => {
    const tab = state.groups[state.active_group === "First" ? 0 : 1]?.active;
    return tab === undefined ? null : tabEditorStates.caretOf(tab);
  }, []);

  const runAction = useCallback(
    async (gen: number, input: WorkspaceActionInput): Promise<Transition | null> => {
      const current = clientRef.current;
      const state = stateRef.current;
      if (current === null || state === null || gen !== generation.current) return null;

      const action = attachLeaving(input, caretOfActive(state));
      const sourceTab =
        input.kind === "Split" ? viewRef.current?.active?.tab : undefined;
      const transition = await current.workspaceApply(state, action, widthRef.current);
      if (gen !== generation.current) return null;
      stateRef.current = transition.state;

      for (const tab of transition.closed) tabEditorStates.drop(tab);
      for (const tab of transition.closed) {
        const gone = findTab(viewRef.current, tab);
        if (gone?.kind === "Board") boards.forget(gone.path);
      }
      // Разделение дублирует заметку в новую вкладку: каретка копируется (US3 AS2).
      if (sourceTab !== undefined && transition.focus !== undefined && transition.focus.tab !== sourceTab) {
        tabEditorStates.copy(sourceTab, transition.focus.tab);
      }

      const next = await current.workspaceView(transition.state, widthRef.current);
      if (gen !== generation.current) return null;
      publishView(next);

      const place = transition.focus?.place;
      if (transition.focus !== undefined && place !== undefined) {
        setPendingPlace({ tab: transition.focus.tab, place });
      }
      const refused = transition.refused[0];
      if (refused !== undefined) {
        const title = findTab(next, refused.tab)?.title ?? refused.path;
        const saveState = registryRef.current?.get(refused.path)?.status().save;
        setRefusal({
          tab: refused.tab,
          path: refused.path,
          title,
          reason: refused.reason,
          detail: saveState?.state === "Failed" ? saveState.reason : null,
        });
      } else if (input.kind === "Close" || input.kind === "CloseOthers") {
        setRefusal(null);
      }
      schedulePersist();
      return transition;
    },
    [boards, caretOfActive, publishView, schedulePersist],
  );

  /**
   * Уход с активной доски, чья запись не удалась, молча невозможен
   * (research.md §6): правки остались бы только в памяти окна.
   */
  const leaveGuard = useCallback(async (): Promise<boolean> => {
    const active = viewRef.current?.active;
    if (active === undefined || active.kind !== "Board") return true;
    if (await boards.flush(active.path)) return true;
    setRefusal({
      tab: active.tab,
      path: active.path,
      title: active.name,
      reason: "Unsaved",
      detail: null,
    });
    return false;
  }, [boards]);

  const dispatch = useCallback(
    (input: WorkspaceActionInput): Promise<Transition | null> => {
      const gen = generation.current;
      return enqueue(async () => {
        if (LEAVING_KINDS.has(input.kind) && !(await leaveGuard())) return null;
        return runAction(gen, input);
      });
    },
    [enqueue, leaveGuard, runAction],
  );

  const open = useCallback(
    async (path: string, target?: TextPlace): Promise<void> => {
      await dispatch({ kind: "Open", path, ...(target === undefined ? {} : { target }) });
    },
    [dispatch],
  );

  const activate = useCallback(
    async (tab: TabId): Promise<void> => {
      const transition = await dispatch({ kind: "Activate", tab });
      if (transition !== null) setFocusRequest((count) => count + 1);
    },
    [dispatch],
  );

  /** Дописать то, что вкладка держит в памяти, и сказать, не осталось ли несохранённой доски. */
  const flushTab = useCallback(
    async (tab: TabView): Promise<boolean> => {
      if (tab.kind === "Board") return boards.flush(tab.path);
      await registryRef.current?.get(tab.path)?.flush();
      return true;
    },
    [boards],
  );

  const close = useCallback(
    (tab: TabId): Promise<void> => {
      const gen = generation.current;
      return enqueue(async () => {
        const entry = findTab(viewRef.current, tab);
        if (entry === undefined) return;
        const clean = await flushTab(entry);
        await runAction(gen, { kind: "Close", tab, board_unsaved: !clean });
      });
    },
    [enqueue, flushTab, runAction],
  );

  const closeOthers = useCallback(
    (tab: TabId): Promise<void> => {
      const gen = generation.current;
      return enqueue(async () => {
        const entry = findTab(viewRef.current, tab);
        if (entry === undefined) return;
        const others = allTabs(viewRef.current).filter(
          (other) => other.group === entry.group && other.id !== tab,
        );
        // `CloseOthers` считает доски сохранёнными — они дописываются до вызова.
        // Если запись какой-то не удалась, закрываем по одной, каждую со своим
        // признаком: правки доски в памяти терять нельзя.
        const results = await Promise.all(others.map(async (other) => ({ other, clean: await flushTab(other) })));
        if (results.every((result) => result.clean)) {
          await runAction(gen, { kind: "CloseOthers", tab });
          return;
        }
        for (const { other, clean } of results) {
          await runAction(gen, { kind: "Close", tab: other.id, board_unsaved: !clean });
        }
      });
    },
    [enqueue, flushTab, runAction],
  );

  const moveBy = useCallback(
    (tab: TabId, delta: -1 | 1): Promise<void> => {
      const gen = generation.current;
      return enqueue(async () => {
        const entry = findTab(viewRef.current, tab);
        if (entry === undefined) return;
        const siblings = allTabs(viewRef.current).filter((other) => other.group === entry.group);
        const at = siblings.findIndex((other) => other.id === tab);
        const index = at + delta;
        if (at < 0 || index < 0 || index >= siblings.length) return;
        const group: GroupSlot = entry.group;
        await runAction(gen, { kind: "Move", tab, group, index });
      });
    },
    [enqueue, runAction],
  );

  const dismissRefusal = useCallback(() => setRefusal(null), []);
  const placeShown = useCallback(() => setPendingPlace(null), []);

  // ── События vault ─────────────────────────────────────────────────────

  const noticeEvent = useCallback(
    (event: VaultEvent) => {
      const gen = generation.current;
      switch (event.type) {
        case "NoteRenamed":
          // Документ меняет ключ сразу: вид под новым путём получает тот же объект.
          registryRef.current?.rename(event.from_path, event.to_path);
          void enqueue(() =>
            runAction(gen, { kind: "Renamed", from: event.from_path, to: event.to_path }),
          ).catch(() => undefined);
          break;
        case "NoteDeleted":
          void enqueue(() => runAction(gen, { kind: "Deleted", path: event.path })).catch(
            () => undefined,
          );
          break;
        case "NoteUpdated":
          // Внешнее изменение заметки без несохранённых правок перечитывает документ
          // во всех его видах; с правками решает ядро (конфликт).
          registryRef.current?.reload(event.path);
          if (allTabs(viewRef.current).some((tab) => tab.path === event.path)) refresh();
          break;
        case "BulkChangeSettled":
          refresh();
          break;
        default:
          break;
      }
    },
    [enqueue, refresh, runAction],
  );

  // ── Загрузка и сброс при смене клиента ────────────────────────────────

  useEffect(() => {
    generation.current += 1;
    const gen = generation.current;
    queueRef.current = Promise.resolve();
    stateRef.current = null;
    vaultRef.current = null;
    viewRef.current = null;
    recentPaths.current = [];
    setView(null);
    setRefusal(null);
    setPendingPlace(null);
    tabEditorStates.clear();
    if (client === null) return undefined;

    void enqueue(async () => {
      let state = await client.workspaceEmpty();
      try {
        const loaded = (await store?.load()) ?? null;
        if (loaded !== null) {
          vaultRef.current = loaded.vault;
          if (loaded.saved !== null) state = await client.workspaceRestore(loaded.saved);
        }
      } catch {
        // Набор не прочитан — vault открывается без вкладок (FR-023).
      }
      if (gen !== generation.current) return;
      stateRef.current = state;
      await readView(gen);
    }).catch(() => undefined);

    return () => {
      // Смена vault и закрытие окна: набор уходит на устройство сразу (US4 AS3).
      void persistNow(client);
      generation.current += 1;
    };
  }, [client, store, enqueue, readView, persistNow]);

  // Уход страницы в фон — последняя надёжная возможность записать набор.
  useEffect(() => {
    const onHidden = (): void => {
      if (document.visibilityState === "hidden") void persistNow();
    };
    document.addEventListener("visibilitychange", onHidden);
    window.addEventListener("pagehide", onHidden);
    return () => {
      document.removeEventListener("visibilitychange", onHidden);
      window.removeEventListener("pagehide", onHidden);
    };
  }, [persistNow]);

  // Ширина окна: перечитываем показ, когда она устоялась. Пороги подачи знает
  // ядро — здесь ни одного числа.
  const lastWidth = useRef(widthPx);
  useEffect(() => {
    if (widthPx === lastWidth.current) return undefined;
    lastWidth.current = widthPx;
    const timer = window.setTimeout(refresh, WIDTH_SETTLE_MS);
    return () => window.clearTimeout(timer);
  }, [widthPx, refresh]);

  useEffect(
    () => () => {
      if (refreshTimer.current !== null) window.clearTimeout(refreshTimer.current);
    },
    [],
  );

  return {
    view,
    dispatch,
    open,
    close,
    closeOthers,
    moveBy,
    activate,
    refusal,
    dismissRefusal,
    pendingPlace,
    placeShown,
    focusRequest,
    noticeEvent,
    refresh,
  };
}
