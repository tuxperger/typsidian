import { useCallback, useEffect, useState } from "react";
import type {
  ApplyPlan,
  Diagnostics,
  Settings,
  SettingsChange,
  SettingsEvent,
  StorageState,
  StoreCapabilities,
  VaultOverrides,
  VaultRecord,
} from "../wasm-core/typsidian_core.js";
import type { DeviceSettingsClient } from "./device-settings-client.js";
import { announceLanguageChoice } from "../i18n/device.js";

// Принцип I конституции: этот хук не переизобретает логику настроек — он только
// вызывает ядро (contracts/settings-store-trait.md) и разбирает уже готовые
// `SettingsEvent` (contracts/frontend-settings.md). Решение о допустимости
// значений и об атомарности сброса принимает ядро; здесь — исполнение
// возвращённого плана и отрисовка.
//
// Строится по образцу `../vault-tree/useVault.ts`.

/**
 * Интервал сверки хранилища: ≤1 с, чтобы уложиться в бюджет SC-014 (≤2 с). В
 * вебе внешнее изменение приходит только из другой вкладки того же
 * происхождения (research.md §3); на native наблюдатель ядра лишь взводит
 * флаг, и опрос забирает уже готовое событие.
 */
export const SETTINGS_POLL_INTERVAL_MS = 1000;

/** Что показать пользователю о состоянии хранилища (FR-005, FR-013, FR-014). */
export interface StorageNotice {
  kind: "unreadable" | "readOnly" | "invalidSetAside";
  /** Куда перенесён испорченный файл — только для `invalidSetAside`. */
  movedTo?: string;
}

export interface SettingsState {
  settings: Settings | null;
  capabilities: StoreCapabilities | null;
  storageState: StorageState | null;
  knownVaults: VaultRecord[];
  /** Сведения раздела «Дополнительно» (FR-039). Собирает ядро, не UI. */
  diagnostics: Diagnostics | null;
  notices: StorageNotice[];
  /** Последний отказ применения — причину показывает панель (FR-016). */
  lastPlan: ApplyPlan | null;
  isLoading: boolean;
}

const INITIAL_STATE: SettingsState = {
  settings: null,
  capabilities: null,
  storageState: null,
  knownVaults: [],
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
};

/** Собирает набор изменений из названных полей, остальные — «не трогать». */
export function changeOf(fields: Partial<SettingsChange>): SettingsChange {
  return { ...NO_CHANGE, ...fields };
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

export function useSettings(client: DeviceSettingsClient): {
  state: SettingsState;
  /**
   * Применить изменение. Возвращает план, вернувшийся из ядра: отказ
   * показывает вызывающая сторона (contracts/frontend-settings.md).
   */
  apply: (change: SettingsChange) => Promise<ApplyPlan | null>;
  /**
   * Сброс к умолчаниям (FR-037). Набор изменений собирает ядро, а применяется
   * он тем же `apply`, что и обычное изменение — поэтому наследует все его
   * предусловия: неразрешённый конфликт отменяет сброс целиком, а список
   * известных vault не затрагивается (FR-038, SC-011).
   */
  resetToDefaults: () => Promise<ApplyPlan | null>;
  /**
   * Сделать действующие сейчас значения умолчаниями устройства (фича 006,
   * FR-031).
   *
   * Единственный способ изменить умолчания: обычная правка при открытом vault
   * уходит в сам vault и устройства не касается (FR-029). Набор изменений
   * собирает ядро по перекрытиям vault — здесь только применение.
   */
  makeSettingsDefault: (
    overrides: VaultOverrides,
  ) => Promise<ApplyPlan | null>;
  refresh: () => Promise<void>;
  dismissNotice: (kind: StorageNotice["kind"]) => void;
} {
  const [state, setState] = useState<SettingsState>(INITIAL_STATE);

  // Какой границей идёт вызов — wasm-bindgen или команда Tauri — хук не знает:
  // очередь к объекту ядра на web и блокировка хранилища на native живут в
  // реализациях `DeviceSettingsClient`.
  const refresh = useCallback(async () => {
    const snapshot = await client.snapshot();
    setState((prev) => ({ ...prev, ...snapshot }));
  }, [client]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const snapshot = await client.snapshot();
        if (cancelled) return;
        setState({ ...INITIAL_STATE, ...snapshot, isLoading: false });
      } catch {
        // Хранилище не ответило вовсе (отказ IPC или wasm). Язык остаётся
        // системным — выбора не прочли, — а панель говорит о недоступности на
        // нём, а не висит в загрузке (010 FR-029).
        if (cancelled) return;
        setState({
          ...INITIAL_STATE,
          isLoading: false,
          notices: [{ kind: "unreadable" }],
        });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  // Сверка с диском и разбор событий (FR-013, SC-014). Опрос, а не подписка:
  // ядро отдаёт события pull-методом, чтобы не требовать object-safe Stream.
  useEffect(() => {
    if (state.isLoading) return;
    let cancelled = false;
    let inFlight = false;

    const tick = async (): Promise<void> => {
      if (inFlight || cancelled) return;
      inFlight = true;
      try {
        const events: SettingsEvent[] = await client.pollEvents();
        if (cancelled || events.length === 0) return;
        setState((prev) => events.reduce(reduceSettingsEvent, prev));
      } catch {
        // Хранилище не ответило на этот раз — следующий тик спросит снова.
      } finally {
        inFlight = false;
      }
    };

    const timer = window.setInterval(() => void tick(), SETTINGS_POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [state.isLoading, client]);

  // Язык интерфейса следует за действующим выбором (фича 010, FR-012): после
  // применения, сброса и правки файла руками — без перезапуска.
  const languageChoice = state.settings?.language;
  const settingsLoaded = state.settings !== null;
  useEffect(() => {
    if (settingsLoaded) announceLanguageChoice(languageChoice);
  }, [settingsLoaded, languageChoice]);

  const apply = useCallback(
    async (change: SettingsChange): Promise<ApplyPlan | null> => {
      const { plan, snapshot } = await client.apply(change);
      setState((prev) => ({ ...prev, ...snapshot, lastPlan: plan }));
      return plan;
    },
    [client],
  );

  // Набор изменений строит ядро, а не этот хук: собрать его здесь значило бы
  // завести умолчания во втором месте (принцип I).
  const resetToDefaults = useCallback(
    async (): Promise<ApplyPlan | null> => apply(await client.resetChange()),
    [apply, client],
  );

  // Набор строит ядро по тем же слоям, что применяет `resolve` — здесь
  // только применение, тем же `apply`, что и любое другое изменение.
  const makeSettingsDefault = useCallback(
    async (overrides: VaultOverrides): Promise<ApplyPlan | null> =>
      apply(await client.defaultsChange(overrides)),
    [apply, client],
  );

  const dismissNotice = useCallback((kind: StorageNotice["kind"]) => {
    setState((prev) => ({
      ...prev,
      notices: prev.notices.filter((notice) => notice.kind !== kind),
    }));
  }, []);

  return {
    state,
    apply,
    resetToDefaults,
    makeSettingsDefault,
    refresh,
    dismissNotice,
  };
}
