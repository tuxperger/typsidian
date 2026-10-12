import { invoke } from "../../tauri-invoke.js";
import type { Settings } from "../../wasm-core/typsidian_core.js";
import { changeOf } from "../../settings/useSettings.js";
import type { WorkspaceTabsStore } from "./tabs-store.js";

/**
 * Набор вкладок на рабочем столе и Android: `settings.json` устройства за
 * командами оболочки (образец — `sync-client-tauri.ts`).
 */
export function tauriTabsStore(): WorkspaceTabsStore {
  return {
    async load() {
      const snapshot = await invoke<{ settings: Settings }>("device_settings");
      const vault = snapshot.settings.current_vault;
      if (!vault) return null;
      const entry = (snapshot.settings.workspaces ?? []).find((item) => item.vault === vault);
      return { vault, saved: entry?.value ?? null };
    },
    async save(vault, saved) {
      await invoke("apply_device_settings", {
        change: changeOf({ workspace: { vault, value: saved } }),
      });
    },
  };
}
