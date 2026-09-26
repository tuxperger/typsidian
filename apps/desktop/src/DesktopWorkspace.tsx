import { VaultSettingsPanel } from "@typsidian/ui/settings/VaultSettingsPanel.js";
import { lastTauriVault } from "@typsidian/ui/vault-client-tauri.js";
import { Workspace } from "@typsidian/ui/workspace/Workspace.js";
import { openDesktopVault } from "./openDesktopVault.js";

/**
 * Оболочка рабочего стола (001 T112).
 *
 * Ровно то же, что делает `apps/web/app/VaultWorkspace.tsx`, и ровно с той же
 * рабочей областью: различие — один проп. Backend приходит не из WASM, а из
 * команд Tauri поверх `NativeBackend` (T113), события — push-ом через канал
 * событий окна (T114), папка выбирается нативным диалогом ОС (T115).
 */
export function DesktopWorkspace(): React.JSX.Element {
  return (
    <Workspace
      openVaultFolder={openDesktopVault}
      // FR-007: последний открытый vault предлагается на экране приветствия.
      // Путь к нему хранит `settings.json` в каталоге конфигурации ОС — то же
      // хранилище, что и на Android, и те же команды оболочки.
      lastVault={lastTauriVault}
      // Не `SettingsPanel`, как на web: та работает через `WebStore` (OPFS), и
      // здесь писала бы настройки в песочницу окна вместо каталога
      // конфигурации ОС. `VaultSettingsPanel` показывает только то, что живёт
      // внутри папки заметок и потому доступно этой сборке по-настоящему.
      settings={(client, _reopen, preview) => (
        <VaultSettingsPanel client={client} preview={preview} />
      )}
    />
  );
}
