import { SettingsPanel } from "@typsidian/ui/settings/SettingsPanel.js";
import { tauriDeviceSettings } from "@typsidian/ui/settings/device-settings-tauri.js";
import { lastTauriVault, openTauriVaultById } from "@typsidian/ui/vault-client-tauri.js";
import { Workspace } from "@typsidian/ui/workspace/Workspace.js";
import { openDesktopVault, pickDesktopFolder } from "./openDesktopVault.js";

/**
 * Хранилище настроек устройства: `settings.json` в каталоге конфигурации ОС
 * за командами оболочки. Один объект на окно — панель подписывается на него
 * эффектами, и новый на каждый рендер перезапускал бы их.
 */
const device = tauriDeviceSettings(pickDesktopFolder);

/**
 * Оболочка рабочего стола (001 T112).
 *
 * Ровно то же, что делает `apps/web/app/VaultWorkspace.tsx`, и ровно с той же
 * рабочей областью и той же панелью настроек: различия — в клиентах. Backend
 * приходит не из WASM, а из команд Tauri поверх `NativeBackend` (T113),
 * события — push-ом через канал событий окна (T114), папка выбирается нативным
 * диалогом ОС (T115), настройки устройства живут в `NativeStore`.
 */
export function DesktopWorkspace(): React.JSX.Element {
  return (
    <Workspace
      openVaultFolder={openDesktopVault}
      // FR-007: последний открытый vault предлагается на экране приветствия.
      // Путь к нему хранит `settings.json` в каталоге конфигурации ОС — то же
      // хранилище, что и на Android, и те же команды оболочки.
      lastVault={lastTauriVault}
      settings={(client, reopen, preview) => (
        <SettingsPanel
          device={device}
          vaultClient={client}
          preview={preview}
          onVaultSwitched={(id) => reopen(() => openTauriVaultById(id))}
        />
      )}
    />
  );
}
