import { DeviceI18nProvider } from "@typsidian/ui/i18n/device.js";
import { SettingsPanel } from "@typsidian/ui/settings/SettingsPanel.js";
import { tauriDeviceSettings } from "@typsidian/ui/settings/device-settings-tauri.js";
import {
  lastTauriVault,
  openTauriVaultById,
  openVaultAt,
} from "@typsidian/ui/vault-client-tauri.js";
import { tauriVaultCloner } from "@typsidian/ui/sync/sync-client-tauri.js";
import { Workspace } from "@typsidian/ui/workspace/Workspace.js";
import { tauriTabsStore } from "@typsidian/ui/workspace/tabs/tabs-store-tauri.js";
import { createTauriShowHost } from "@typsidian/ui/slides/show-channel-tauri.js";
import { pathImportPicker } from "@typsidian/ui/import/import-source.js";
import {
  openDesktopVault,
  pickDesktopFolder,
  pickDesktopImportFolder,
} from "./openDesktopVault.js";

/**
 * Хранилище настроек устройства: `settings.json` в каталоге конфигурации ОС
 * за командами оболочки. Один объект на окно — панель подписывается на него
 * эффектами, и новый на каждый рендер перезапускал бы их.
 */
const device = tauriDeviceSettings(pickDesktopFolder);

/**
 * Создание vault из удалённого репозитория (фича 011): пустую папку выбирает
 * тот же диалог ОС, что и папку vault.
 */
const cloner = tauriVaultCloner(pickDesktopFolder, openVaultAt);

/** Набор вкладок vault в `settings.json` устройства (фича 014). */
const tabsStore = tauriTabsStore();

/** Хозяин окон показа слайдов (фича 019): `WebviewWindow` + события Tauri. */
const showHost = createTauriShowHost();

/** Выбор папки Obsidian для импорта (фича 021): диалог ОС, дальше — путь. */
const importPicker = pathImportPicker(pickDesktopImportFolder);

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
    <DeviceI18nProvider device={device}>
      <Workspace
        openVaultFolder={openDesktopVault}
        // FR-007: последний открытый vault предлагается на экране приветствия.
        // Путь к нему хранит `settings.json` в каталоге конфигурации ОС — то же
        // хранилище, что и на Android, и те же команды оболочки.
        lastVault={lastTauriVault}
        cloneVault={cloner}
        // Состояние колонки заметки (фича 013) хранится в настройках устройства.
        device={device}
        // Набор вкладок и разделение переживают перезапуск (фича 014).
        tabsStore={tabsStore}
        showHost={showHost}
        // Импорт из Obsidian (фича 021): папка выбирается диалогом ОС и открывается только для чтения.
        pickImportSource={importPicker}
        settings={(client, reopen, preview, extras) => (
          <SettingsPanel
            device={device}
            vaultClient={client}
            preview={preview}
            sync={extras.sync}
            onCloneVault={extras.onCloneVault}
            onVaultSwitched={(id) => reopen(() => openTauriVaultById(id))}
          />
        )}
      />
    </DeviceI18nProvider>
  );
}
