import type { SavedWorkspace, VaultId } from "../../wasm-core/typsidian_core.js";

/**
 * Хранилище набора вкладок на устройстве (фича 014, FR-021…FR-023).
 *
 * Порт, как `lastVault`: сам набор лежит в настройках устройства — вне vault,
 * вне его истории и доставки (FR-022), — а как до них добраться, знает оболочка:
 * `WebStore` поверх OPFS в браузере, команды `device_settings` и
 * `apply_device_settings` на рабочем столе и Android. Что считать годным набором
 * и как восстанавливать его по существующим заметкам, решает ядро; здесь только
 * чтение и запись значения.
 *
 * Не передан или отказал — вкладки работают, но не переживают перезапуск;
 * пользователю об этом не сообщается (FR-023).
 */
export interface WorkspaceTabsStore {
  /**
   * Набор текущего vault. `null` — vault не опознан (не запомнился), сохранять
   * некуда. Идентификатор возвращается вместе с набором: запись после смены vault
   * не должна попасть в чужую запись (US4 AS3).
   */
  load(): Promise<{ vault: VaultId; saved: SavedWorkspace | null } | null>;
  save(vault: VaultId, saved: SavedWorkspace): Promise<void>;
}
