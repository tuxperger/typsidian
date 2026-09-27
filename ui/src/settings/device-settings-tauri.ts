import { invoke } from "@tauri-apps/api/core";
import type {
  ApplyPlan,
  ChangeTarget,
  Settings,
  SettingsChange,
  SettingsEvent,
} from "../wasm-core/typsidian_core.js";
import type { DeviceSettingsClient, DeviceSnapshot } from "./device-settings-client.js";

/**
 * Хранилище настроек устройства на рабочем столе и Android — `NativeStore`
 * ядра за командами оболочки (`packages/shell`): `settings.json` в каталоге
 * конфигурации ОС, то же, откуда приходит последний открытый vault.
 *
 * Выбор папки передаёт оболочка: на рабочем столе это диалог ОС, на Android —
 * обзор, который рисует само окно. `null` из него — пользователь передумал.
 */
export function tauriDeviceSettings(
  pickFolder: () => Promise<string | null>,
): DeviceSettingsClient {
  return {
    async snapshot() {
      return fromShell(await invoke<ShellSnapshot>("device_settings"));
    },
    pollEvents() {
      return invoke<SettingsEvent[]>("device_settings_events");
    },
    async apply(change) {
      const applied = await invoke<{ plan: ApplyPlan; snapshot: ShellSnapshot }>(
        "apply_device_settings",
        { change },
      );
      return { plan: applied.plan, snapshot: fromShell(applied.snapshot) };
    },
    resetChange() {
      return invoke<SettingsChange>("settings_reset_change");
    },
    changeTarget(change, vaultIsOpen) {
      return invoke<ChangeTarget>("settings_change_target", { change, vaultIsOpen });
    },
    resolveWithVault(overrides) {
      return invoke<Settings>("resolve_device_settings", { overrides });
    },
    defaultsChange(overrides) {
      return invoke<SettingsChange>("device_defaults_change", { overrides });
    },
    async reveal() {
      await invoke("reveal_device_settings");
    },
    async addFolder() {
      const path = await pickFolder();
      if (path === null) return { kind: "cancelled" };
      const id = await invoke<string>("add_known_vault", { path });
      return { kind: "added", id };
    },
    async setVaultLabel(id, label) {
      await invoke("set_known_vault_label", { id, label });
    },
    async forgetVault(id) {
      await invoke("forget_known_vault", { id });
    },
  };
}

/** Ответ `device_settings` — `DeviceSnapshot` оболочки, поля в snake_case. */
interface ShellSnapshot {
  settings: DeviceSnapshot["settings"];
  capabilities: DeviceSnapshot["capabilities"];
  storage_state: DeviceSnapshot["storageState"];
  diagnostics: DeviceSnapshot["diagnostics"];
  known_vaults: DeviceSnapshot["knownVaults"];
}

function fromShell(snapshot: ShellSnapshot): DeviceSnapshot {
  return {
    settings: snapshot.settings,
    capabilities: snapshot.capabilities,
    storageState: snapshot.storage_state,
    diagnostics: snapshot.diagnostics,
    knownVaults: snapshot.known_vaults,
  };
}
