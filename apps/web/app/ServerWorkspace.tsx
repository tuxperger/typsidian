"use client";

import { SettingsPanel } from "@typsidian/ui/settings/SettingsPanel.js";
import { ServerApp } from "@typsidian/ui/server/ServerApp.js";
import { Workspace } from "@typsidian/ui/workspace/Workspace.js";
import { device, showHost, tabsStore } from "./web-device";

/**
 * Web в серверном режиме (фича 037): та же рабочая область, что с папкой на
 * устройстве, но vault лежит на своём сервере, а клиент — команды Tauri поверх
 * HTTP (`@typsidian/ui/server`). Импорта из папки Obsidian нет: папку-источник
 * сервер открыть не может (research.md §4).
 */
export function ServerWorkspace({ onDeviceMode }: { onDeviceMode: () => void }): React.JSX.Element {
  return (
    <ServerApp
      onDeviceMode={onDeviceMode}
      renderWorkspace={({ lastVault, openVaultFolder, account }) => (
        <Workspace
          openVaultFolder={openVaultFolder}
          lastVault={lastVault}
          device={device}
          tabsStore={tabsStore}
          showHost={showHost}
          settings={(client, _reopen, preview, extras) => (
            <SettingsPanel
              device={device}
              vaultClient={client}
              preview={preview}
              sync={extras.sync}
              account={account}
            />
          )}
        />
      )}
    />
  );
}
