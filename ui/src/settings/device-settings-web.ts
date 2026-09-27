import {
  WebStore,
  type SettingsEvent,
  type VaultRecord,
} from "../wasm-core/typsidian_core.js";
import type { DeviceSettingsClient, DeviceSnapshot } from "./device-settings-client.js";
import { deviceStore, deviceStoreQueue } from "./deviceStore.js";

/**
 * Хранилище настроек устройства в браузере — `WebStore` поверх OPFS и
 * IndexedDB.
 *
 * Все обращения к экземпляру идут через `deviceStoreQueue`: `async`-методы
 * `WebStore` с `&mut self` держат заимствование объекта до конца своей future,
 * и наложение опроса на жест пользователя падает с «recursive use of an object
 * detected» (см. `wasm-call-queue.ts`). Синхронные геттеры читаются внутри той
 * же операции очереди, а не рядом с ней.
 */
export function webDeviceSettings(): DeviceSettingsClient {
  const queue = deviceStoreQueue;

  const snapshotOf = async (store: WebStore): Promise<DeviceSnapshot> => ({
    knownVaults: (await store.knownVaults()) as VaultRecord[],
    settings: store.settings(),
    capabilities: store.capabilities(),
    storageState: store.storageState(),
    diagnostics: store.diagnostics(),
  });

  return {
    async snapshot() {
      const store = await deviceStore();
      return queue.run(() => snapshotOf(store));
    },
    async pollEvents() {
      const store = await deviceStore();
      return (await queue.runInternal(() => store.pollEvents())) as SettingsEvent[];
    },
    async apply(change) {
      const store = await deviceStore();
      return queue.run(async () => {
        const plan = await store.apply(change);
        return { plan, snapshot: await snapshotOf(store) };
      });
    },
    async resetChange() {
      await deviceStore();
      return WebStore.resetToDefaults();
    },
    async changeTarget(change, vaultIsOpen) {
      await deviceStore();
      return WebStore.changeTarget(change, vaultIsOpen);
    },
    async resolveWithVault(overrides) {
      const store = await deviceStore();
      return queue.run(async () => store.resolveWithVault(overrides));
    },
    async defaultsChange(overrides) {
      const store = await deviceStore();
      return queue.run(async () => store.deviceDefaultsChange(overrides));
    },
    async reveal() {
      const store = await deviceStore();
      await queue.run(async () => store.reveal());
    },
    async addFolder() {
      const picker = (
        window as unknown as {
          showDirectoryPicker?: () => Promise<FileSystemDirectoryHandle>;
        }
      ).showDirectoryPicker;
      if (!picker) return { kind: "unsupported" };
      let handle: FileSystemDirectoryHandle;
      try {
        // Жест пользователя: `showDirectoryPicker` нельзя вызвать из ядра —
        // по той же причине, что и в `vault-tree/useVault.ts`.
        handle = await picker();
      } catch (cause) {
        if (cause instanceof DOMException && cause.name === "AbortError") {
          return { kind: "cancelled" };
        }
        throw cause;
      }
      const store = await deviceStore();
      // Ссылка передаётся ядру **не разбирая** её. Повторное добавление той же
      // папки ядро само сведёт к существующей записи (FR-020), а две разные
      // папки с одинаковым именем останутся двумя записями.
      const id = await queue.run(() => store.addVault(handle, handle.name));
      return { kind: "added", id };
    },
    async setVaultLabel(id, label) {
      const store = await deviceStore();
      await queue.run(() => store.setVaultLabel(id, label ?? undefined));
    },
    async forgetVault(id) {
      const store = await deviceStore();
      await queue.run(() => store.forgetVault(id));
    },
  };
}
