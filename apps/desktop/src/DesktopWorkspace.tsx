import { Workspace } from "@typsidian/ui/workspace/Workspace.js";
import { openTauriVault } from "./tauriVaultClient.js";

/**
 * Оболочка рабочего стола (001 T112).
 *
 * Ровно то же, что делает `apps/web/app/VaultWorkspace.tsx`, и ровно с той же
 * рабочей областью: различие — один проп. Backend приходит не из WASM, а из
 * команд Tauri поверх `NativeBackend` (T113), события — push-ом через канал
 * событий окна (T114), папка выбирается нативным диалогом ОС (T115).
 */
export function DesktopWorkspace(): React.JSX.Element {
  return <Workspace openVaultFolder={openTauriVault} />;
}
