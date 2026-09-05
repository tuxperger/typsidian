"use client";

import { SettingsPanel } from "@typsidian/ui/settings/SettingsPanel.js";
import { openWebVault } from "@typsidian/ui/vault-client-web.js";
import { Workspace } from "@typsidian/ui/workspace/Workspace.js";

/**
 * Оболочка web-клиента (plan.md → Project Structure).
 *
 * От неё требуется ровно одно: сказать общей рабочей области, что на этом
 * клиенте платформенное. В браузере это жест `showDirectoryPicker` вместе с
 * `WebBackend` поверх File System Access API и настройки поверх `WebStore`
 * (OPFS). На рабочем столе тот же `Workspace` получает клиента поверх команд
 * Tauri (001 T112).
 */
export function VaultWorkspace(): React.JSX.Element {
  return <Workspace openVaultFolder={openWebVault} settings={<SettingsPanel />} />;
}
