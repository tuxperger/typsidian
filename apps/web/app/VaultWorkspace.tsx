"use client";

import { SettingsPanel } from "@typsidian/ui/settings/SettingsPanel.js";
import {
  lastWebVault,
  openWebVault,
  openWebVaultById,
} from "@typsidian/ui/vault-client-web.js";
import { Workspace } from "@typsidian/ui/workspace/Workspace.js";

/**
 * Оболочка web-клиента (plan.md → Project Structure).
 *
 * От неё требуется ровно одно: сказать общей рабочей области, что на этом
 * клиенте платформенное. В браузере это жест `showDirectoryPicker` вместе с
 * `WebBackend` поверх File System Access API, настройки поверх `WebStore`
 * (OPFS) и компилятор предпросмотра как wasm в том же процессе. На рабочем
 * столе тот же `Workspace` получает клиентов поверх команд Tauri (001 T112).
 *
 * Компилятор предпросмотра здесь больше не создаётся отдельно: с появлением
 * фазы резолва (005 T014-T015) сборка читает связанные файлы через тот же
 * `WebBackend` и обязана делить с ним очередь вызовов ядра, поэтому оба
 * клиента приходят из `openWebVault` уже связанными.
 */
export function VaultWorkspace(): React.JSX.Element {
  return (
    <Workspace
      openVaultFolder={openWebVault}
      // FR-007: последний открытый vault предлагается на экране приветствия.
      // Хендл папки хранит `WebStore` в IndexedDB; разрешение на неё
      // переспрашивается уже по нажатию — браузер выдаёт его только в
      // обработчике жеста пользователя.
      lastVault={lastWebVault}
      // Клиент открытого vault нужен разделу «Vault»: политика расположения
      // файлов вставок и помеченные разделы живут внутри vault, а не в
      // хранилище устройства (фича 006, FR-002).
      //
      // `reopen` — вторая половина переключения vault (US2 фичи 004, AS3):
      // панель меняет текущий vault в настройках, а открывает выбранный
      // рабочая область, потому что клиентом владеет она.
      settings={(client, reopen, preview) => (
        <SettingsPanel
          vaultClient={client}
          preview={preview}
          onVaultSwitched={(id) => reopen(() => openWebVaultById(id))}
        />
      )}
    />
  );
}
