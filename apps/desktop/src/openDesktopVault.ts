import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { openVaultAt } from "@typsidian/ui/vault-client-tauri.js";
import type { VaultOpenResult } from "@typsidian/ui/vault-tree/useVault.js";

/**
 * Жест выбора папки на рабочем столе (001 T115).
 *
 * Единственное, чем оболочка рабочего стола отличается от мобильной: сам
 * `VaultClient` поверх команд Tauri общий и живёт в
 * `@typsidian/ui/vault-client-tauri.js`.
 *
 * Выбор делает нативный диалог ОС — это жест пользователя, и ядро его не
 * инициирует: то же правило, по которому `showDirectoryPicker` вызывается в UI
 * (research.md §1).
 */
export async function openDesktopVault(): Promise<VaultOpenResult> {
  let chosen: string | null;
  try {
    const selection = await openFolderDialog({
      directory: true,
      multiple: false,
      title: "Выберите папку с заметками",
    });
    chosen = typeof selection === "string" ? selection : null;
  } catch (cause) {
    return { ok: false, error: { kind: "OpenFailed", message: String(cause) } };
  }
  // `null` — диалог закрыт без выбора. Отмена ошибкой не считается:
  // пользователь передумал (Assumptions).
  if (chosen === null) return { ok: false, error: { kind: "PickerCancelled" } };

  return openVaultAt(chosen);
}
