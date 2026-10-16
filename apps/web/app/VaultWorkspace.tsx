"use client";

import { DeviceI18nProvider } from "@typsidian/ui/i18n/device.js";
import { SettingsPanel } from "@typsidian/ui/settings/SettingsPanel.js";
import { webDeviceSettings } from "@typsidian/ui/settings/device-settings-web.js";
import {
  cloneWebVault,
  lastWebVault,
  openWebVault,
  openWebVaultById,
  webImportPicker,
} from "@typsidian/ui/vault-client-web.js";
import { Workspace } from "@typsidian/ui/workspace/Workspace.js";
import { webTabsStore } from "@typsidian/ui/workspace/tabs/tabs-store-web.js";
import { createWebShowHost } from "@typsidian/ui/slides/show-channel-web.js";

/**
 * Хранилище настроек устройства — `WebStore` поверх OPFS. Один объект на
 * страницу: панель подписывается на него эффектами.
 */
const device = webDeviceSettings();

/** Набор вкладок vault на устройстве (фича 014): тот же `WebStore`, что и у настроек. */
const tabsStore = webTabsStore();

/** Хозяин окон показа слайдов (фича 019): `window.open` + `BroadcastChannel`. */
const showHost = createWebShowHost();

/** Выбор исходной папки импорта из Obsidian (фича 021): `showDirectoryPicker` в режиме чтения. */
const importPicker = webImportPicker();

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
    <DeviceI18nProvider device={device}>
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
        cloneVault={cloneWebVault}
        // Состояние колонки заметки (фича 013) хранится в настройках устройства.
        device={device}
        // Набор вкладок и разделение переживают перезапуск (фича 014).
        tabsStore={tabsStore}
        // Показ слайдов в отдельном окне, которое можно вынести на проектор (019).
        showHost={showHost}
        // Импорт из Obsidian (фича 021); без File System Access API диалог называет причину.
        pickImportSource={importPicker}
        settings={(client, reopen, preview, extras) => (
          <SettingsPanel
            device={device}
            vaultClient={client}
            preview={preview}
            sync={extras.sync}
            onCloneVault={extras.onCloneVault}
            onVaultSwitched={(id) => reopen(() => openWebVaultById(id))}
          />
        )}
      />
    </DeviceI18nProvider>
  );
}
