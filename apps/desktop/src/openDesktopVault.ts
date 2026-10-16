import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { openVaultAt } from "@typsidian/ui/vault-client-tauri.js";
import type { VaultOpenResult } from "@typsidian/ui/vault-tree/useVault.js";
import { currentI18n } from "@typsidian/ui/i18n/index.js";

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
    chosen = await pickDesktopFolder();
  } catch (cause) {
    return { ok: false, error: { kind: "OpenFailed", detail: String(cause) } };
  }
  // `null` — диалог закрыт без выбора. Отмена ошибкой не считается:
  // пользователь передумал (Assumptions).
  if (chosen === null) return { ok: false, error: { kind: "PickerCancelled" } };

  return openVaultAt(chosen);
}

/**
 * Диалог ОС «выберите папку». `null` — закрыт без выбора.
 *
 * Отдельно от открытия: тем же диалогом панель настроек добавляет папку в
 * список известных vault, не открывая её.
 */
export async function pickDesktopFolder(): Promise<string | null> {
  const selection = await openFolderDialog({
    directory: true,
    multiple: false,
    // Язык — на момент вызова: диалог рисует ОС, перерисовать его нельзя.
    title: currentI18n().t("shell.pickFolderTitle"),
  });
  return typeof selection === "string" ? selection : null;
}

/**
 * Диалог ОС «выберите папку с заметками Obsidian» (фича 021). Тот же диалог,
 * что и для vault, со своим заголовком; сам импорт открывает папку только для
 * чтения командой `import_open_source`. `null` — закрыт без выбора.
 */
export async function pickDesktopImportFolder(): Promise<string | null> {
  const selection = await openFolderDialog({
    directory: true,
    multiple: false,
    title: currentI18n().t("import.pickTitle"),
  });
  return typeof selection === "string" ? selection : null;
}
