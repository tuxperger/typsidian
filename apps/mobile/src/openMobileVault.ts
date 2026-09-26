import { invoke } from "@tauri-apps/api/core";
import { openVaultAt } from "@typsidian/ui/vault-client-tauri.js";
import type { VaultOpenResult } from "@typsidian/ui/vault-tree/useVault.js";

/**
 * Жест выбора папки на Android.
 *
 * Единственное, чем мобильная оболочка отличается от рабочего стола: сам
 * `VaultClient` поверх команд Tauri общий и живёт в
 * `@typsidian/ui/vault-client-tauri.js`.
 *
 * Системного диалога выбора папки на Android нет — ни у Tauri (`pick_folder`
 * плагина реализован только под desktop), ни в виде, пригодном ядру: SAF
 * отдаёт `content://`-адреса, которых `NativeBackend` не понимает, он построен
 * на `std::fs`. Поэтому обзор папок рисует само окно (`FolderPicker`), а сюда
 * приходит уже выбранный путь.
 *
 * Раньше выбора не было вовсе: путь был один и фиксированный,
 * `Documents/Typsidian`, и папку с заметками, лежащую где-то ещё (карта памяти,
 * каталог синхронизации), открыть было нельзя.
 */
export async function openMobileVault(
  pick: () => Promise<string | null>,
): Promise<VaultOpenResult> {
  const path = await pick();
  // `null` — обзор закрыт без выбора. Отмена ошибкой не считается:
  // пользователь передумал (Assumptions).
  if (path === null) return { ok: false, error: { kind: "PickerCancelled" } };

  // Запоминание последнего vault делает `openVaultAt` — одинаково для Android и
  // рабочего стола (FR-007).
  return openVaultAt(path);
}

/**
 * С какой папки открыть обзор.
 *
 * Последний открытый vault из хранилища настроек устройства — то же, что
 * предлагает экран приветствия, только здесь нужен сам путь, а не подпись.
 * `null` — обзор начнётся с корня общей памяти.
 *
 * Своего хранилища у этой оболочки больше нет: прежняя запись в `localStorage`
 * окна была вторым источником истины об одном и том же факте и не переживала
 * очистку данных приложения.
 */
export async function lastMobileFolder(): Promise<string | null> {
  try {
    const record = await invoke<{ path: string } | null>("last_vault");
    return record?.path ?? null;
  } catch {
    return null;
  }
}
