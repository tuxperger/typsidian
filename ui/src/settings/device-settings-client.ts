import type {
  ApplyPlan,
  ChangeTarget,
  Diagnostics,
  Settings,
  SettingsChange,
  SettingsEvent,
  StorageState,
  StoreCapabilities,
  VaultOverrides,
  VaultRecord,
} from "../wasm-core/typsidian_core.js";

/**
 * Хранилище настроек устройства глазами панели настроек (фича 004).
 *
 * Та же форма, что у `VaultClient`: одна панель, две реализации. На web — это
 * `WebStore` поверх OPFS и IndexedDB (`device-settings-web.ts`), на рабочем
 * столе и Android — `NativeStore` за командами Tauri
 * (`device-settings-tauri.ts`). Раньше панель импортировала `WebStore`
 * напрямую, и нативным оболочкам доставалась урезанная панель одних настроек
 * папки — отсюда и расхождение настроек между web и desktop.
 *
 * Модуль — только типы: импорт его значением не должен тянуть wasm в сборку
 * оболочки, которая wasm не грузит.
 *
 * Решений здесь нет (принцип I): куда уходит правка, чем является сброс и как
 * слои складываются в действующие значения, отвечает ядро — клиент только
 * передаёт вопрос через свою границу.
 */
export interface DeviceSettingsClient {
  /** Всё, что читает панель, одним снимком. */
  snapshot(): Promise<DeviceSnapshot>;
  /** События хранилища с прошлого вызова (FR-013). Панель опрашивает их сама. */
  pollEvents(): Promise<SettingsEvent[]>;
  /** Применить набор; план — из ядра, снимок — уже после применения. */
  apply(change: SettingsChange): Promise<{ plan: ApplyPlan; snapshot: DeviceSnapshot }>;
  /** Набор, возвращающий значения к умолчаниям (FR-037). */
  resetChange(): Promise<SettingsChange>;
  /** Куда уходит правка — в устройство или в открытый vault (FR-029). */
  changeTarget(change: SettingsChange, vaultIsOpen: boolean): Promise<ChangeTarget>;
  /** Действующие значения: устройство под перекрытием vault (FR-030). */
  resolveWithVault(overrides: VaultOverrides): Promise<Settings>;
  /** Набор, делающий действующие значения умолчаниями устройства (FR-031). */
  defaultsChange(overrides: VaultOverrides): Promise<SettingsChange>;
  /** Раскрыть хранилище средствами ОС (FR-036). */
  reveal(): Promise<void>;
  /**
   * Жест «добавить папку» и запись её в список известных (FR-020). Выбор папки
   * платформенный — `showDirectoryPicker` в браузере, диалог ОС на рабочем
   * столе, свой обзор на Android, — поэтому он здесь, а не в панели.
   */
  addFolder(): Promise<AddFolderOutcome>;
  setVaultLabel(id: string, label: string | null): Promise<void>;
  /** Забыть запись; папка на диске не трогается (FR-024). */
  forgetVault(id: string): Promise<void>;
}

/** Снимок хранилища устройства. */
export interface DeviceSnapshot {
  settings: Settings;
  capabilities: StoreCapabilities;
  storageState: StorageState;
  diagnostics: Diagnostics;
  knownVaults: VaultRecord[];
}

/** Чем закончился жест «добавить папку». */
export type AddFolderOutcome =
  | { kind: "added"; id: string }
  /** Пользователь закрыл выбор — это не сбой. */
  | { kind: "cancelled" }
  /** Платформа не умеет выбирать папки (браузер без File System Access API). */
  | { kind: "unsupported" };
