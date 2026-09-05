import { useCallback, useEffect, useRef, useState } from "react";
import init, {
  WebStore,
  type ApplyPlan,
  type ConfigDiskState,
  type Diagnostics,
  type EditorMode,
  type Settings,
  type SettingsChange,
  type SettingsEvent,
  type StorageState,
  type StoreCapabilities,
  type VaultRecord,
} from "../wasm-core/typsidian_core.js";

// Принцип I конституции: этот хук не переизобретает логику настроек — он только
// вызывает ядро (contracts/settings-store-trait.md) и разбирает уже готовые
// `SettingsEvent` (contracts/frontend-settings.md). Решение о принудительном
// сохранении, о допустимости переключения и об атомарности сброса принимает
// ядро; здесь — исполнение возвращённой инструкции и отрисовка.
//
// Строится по образцу `../vault-tree/useVault.ts`.

/**
 * Интервал сверки хранилища. В вебе внешнее изменение приходит только из другой
 * вкладки того же происхождения (research.md §3), и обнаруживается оно
 * периодической сверкой: ≤1 с, чтобы уложиться в бюджет SC-014 (≤2 с).
 */
export const SETTINGS_POLL_INTERVAL_MS = 1000;

/** Что показать пользователю о состоянии хранилища (FR-005, FR-013, FR-014). */
export interface StorageNotice {
  kind: "unreadable" | "readOnly" | "invalidSetAside";
  /** Куда перенесён испорченный файл — только для `invalidSetAside`. */
  movedTo?: string;
}

export interface SettingsState {
  store: WebStore | null;
  settings: Settings | null;
  capabilities: StoreCapabilities | null;
  storageState: StorageState | null;
  knownVaults: VaultRecord[];
  editorMode: EditorMode | null;
  configDiskState: ConfigDiskState | null;
  /** Сведения раздела «Дополнительно» (FR-039). Собирает ядро, не UI. */
  diagnostics: Diagnostics | null;
  notices: StorageNotice[];
  /** Последний отказ применения — причину показывает панель (FR-016). */
  lastPlan: ApplyPlan | null;
  isLoading: boolean;
}

const INITIAL_STATE: SettingsState = {
  store: null,
  settings: null,
  capabilities: null,
  storageState: null,
  knownVaults: [],
  editorMode: null,
  configDiskState: null,
  diagnostics: null,
  notices: [],
  lastPlan: null,
  isLoading: true,
};

/**
 * «Ничего не менять». Поля `SettingsChange` порождены из Rust-`Option` как
 * `T | undefined`, а не как необязательные, и при `exactOptionalPropertyTypes`
 * их обязан назвать каждый вызов. Явная база вместо приведения типа: принцип IV
 * запрещает глушить это `as`.
 */
const NO_CHANGE: SettingsChange = {
  theme: undefined,
  editor_font_size: undefined,
  current_vault: undefined,
  editor_mode_preference: undefined,
  editor_config_path: undefined,
};

/** Собирает набор изменений из названных полей, остальные — «не трогать». */
export function changeOf(fields: Partial<SettingsChange>): SettingsChange {
  return { ...NO_CHANGE, ...fields };
}

let wasmInitPromise: Promise<unknown> | null = null;

function ensureWasmInitialized(): Promise<unknown> {
  if (!wasmInitPromise) {
    wasmInitPromise = init();
  }
  return wasmInitPromise;
}

/**
 * Исчерпывающий разбор одного `SettingsEvent` (contracts/frontend-settings.md).
 * `never` в `default` — гейт принципа IV: если ядро добавит вариант, UI не
 * соберётся, пока его здесь не обработают.
 */
export function reduceSettingsEvent(
  state: SettingsState,
  event: SettingsEvent,
): SettingsState {
  switch (event.type) {
    case "ValuesChangedExternally":
      // FR-013, FR-006: применить и обновить панель без перезапуска.
      return { ...state, settings: event.settings };
    case "StorageUnreadable":
      return { ...state, notices: addNotice(state.notices, { kind: "unreadable" }) };
    case "StorageRecovered":
      // Предупреждение снимается, показываются восстановленные значения.
      return {
        ...state,
        settings: event.settings,
        notices: state.notices.filter((notice) => notice.kind !== "unreadable"),
      };
    case "StorageBecameReadOnly":
      return { ...state, notices: addNotice(state.notices, { kind: "readOnly" }) };
    case "InvalidFileSetAside":
      return {
        ...state,
        notices: addNotice(state.notices, {
          kind: "invalidSetAside",
          movedTo: event.moved_to,
        }),
      };
    case "EditorModeChanged":
      return { ...state, editorMode: event.mode };
    case "EditorConfigDiskStateChanged":
      // Показать «изменён, не применён»; применять самостоятельно нельзя
      // (FR-026).
      return { ...state, configDiskState: event.disk_state };
    default: {
      const exhaustive: never = event;
      return exhaustive;
    }
  }
}

function addNotice(notices: StorageNotice[], next: StorageNotice): StorageNotice[] {
  const withoutSameKind = notices.filter((notice) => notice.kind !== next.kind);
  return [...withoutSameKind, next];
}

export function useSettings(): {
  state: SettingsState;
  /**
   * Применить изменение. Возвращает план, вернувшийся из ядра: разбор
   * `RequiresNoteSave` — забота вызывающей стороны (contracts/
   * frontend-settings.md), потому что сохранение заметки выполняется
   * `VaultBackend::write`, о котором настройки ничего не знают.
   */
  apply: (change: SettingsChange) => Promise<ApplyPlan | null>;
  /**
   * Сброс к умолчаниям (FR-037). Набор изменений собирает ядро
   * (`WebStore.resetToDefaults`), а применяется он тем же `apply`, что и
   * обычное изменение — поэтому наследует все его предусловия: неразрешённый
   * конфликт отменяет сброс целиком, а список известных vault не затрагивается
   * (FR-038, SC-011).
   */
  resetToDefaults: () => Promise<ApplyPlan | null>;
  refresh: () => Promise<void>;
  dismissNotice: (kind: StorageNotice["kind"]) => void;
} {
  const [state, setState] = useState<SettingsState>(INITIAL_STATE);
  const storeRef = useRef<WebStore | null>(null);

  const refresh = useCallback(async () => {
    const store = storeRef.current;
    if (!store) return;
    const knownVaults = (await store.knownVaults()) as VaultRecord[];
    setState((prev) => ({
      ...prev,
      settings: store.settings(),
      capabilities: store.capabilities(),
      storageState: store.storageState(),
      diagnostics: store.diagnostics(),
      knownVaults,
    }));
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      await ensureWasmInitialized();
      const store = await WebStore.create();
      if (cancelled) return;
      storeRef.current = store;
      const knownVaults = (await store.knownVaults()) as VaultRecord[];
      if (cancelled) return;
      setState({
        ...INITIAL_STATE,
        store,
        settings: store.settings(),
        capabilities: store.capabilities(),
        storageState: store.storageState(),
        diagnostics: store.diagnostics(),
        knownVaults,
        isLoading: false,
      });
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Сверка с диском и разбор событий (FR-013, SC-014). Опрос, а не подписка:
  // ядро отдаёт события pull-методом, чтобы не требовать object-safe Stream.
  useEffect(() => {
    if (!state.store) return;
    let cancelled = false;
    let inFlight = false;

    const tick = async (): Promise<void> => {
      const store = storeRef.current;
      if (!store || inFlight || cancelled) return;
      inFlight = true;
      try {
        const events = (await store.pollEvents()) as SettingsEvent[];
        if (cancelled || events.length === 0) return;
        setState((prev) => events.reduce(reduceSettingsEvent, prev));
      } finally {
        inFlight = false;
      }
    };

    const timer = window.setInterval(() => void tick(), SETTINGS_POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [state.store]);

  const apply = useCallback(
    async (change: SettingsChange): Promise<ApplyPlan | null> => {
      const store = storeRef.current;
      if (!store) return null;
      const plan = await store.apply(change);
      setState((prev) => ({
        ...prev,
        settings: store.settings(),
        storageState: store.storageState(),
        capabilities: store.capabilities(),
        diagnostics: store.diagnostics(),
        lastPlan: plan,
      }));
      return plan;
    },
    [],
  );

  // Набор изменений строит ядро, а не этот хук: собрать его здесь значило бы
  // завести умолчания во втором месте (принцип I).
  const resetToDefaults = useCallback(
    async (): Promise<ApplyPlan | null> => apply(WebStore.resetToDefaults()),
    [apply],
  );

  const dismissNotice = useCallback((kind: StorageNotice["kind"]) => {
    setState((prev) => ({
      ...prev,
      notices: prev.notices.filter((notice) => notice.kind !== kind),
    }));
  }, []);

  return { state, apply, resetToDefaults, refresh, dismissNotice };
}
