import { deviceStore, deviceStoreQueue } from "../../settings/deviceStore.js";
import { changeOf } from "../../settings/useSettings.js";
import type { WorkspaceTabsStore } from "./tabs-store.js";

/**
 * Набор вкладок в браузере: `WebStore` поверх OPFS (фича 004). Обращения идут
 * через очередь хранилища — оно одно на страницу, и наложение вызовов роняет
 * его «recursive use of an object».
 */
export function webTabsStore(): WorkspaceTabsStore {
  return {
    async load() {
      const store = await deviceStore();
      return deviceStoreQueue.run(async () => {
        const settings = store.settings();
        const vault = settings.current_vault;
        if (!vault) return null;
        const entry = (settings.workspaces ?? []).find((item) => item.vault === vault);
        return { vault, saved: entry?.value ?? null };
      });
    },
    async save(vault, saved) {
      const store = await deviceStore();
      await deviceStoreQueue.run(async () => {
        await store.apply(changeOf({ workspace: { vault, value: saved } }));
      });
    },
  };
}
