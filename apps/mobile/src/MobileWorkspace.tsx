import { useCallback, useEffect, useMemo, useState } from "react";
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
import { FolderPicker } from "./FolderPicker.js";
import { lastMobileFolder, openMobileVault } from "./openMobileVault.js";
import { useFolderPicker } from "./useFolderPicker.js";

/**
 * Мобильная оболочка.
 *
 * Ровно то же, что делают `apps/web/app/VaultWorkspace.tsx` и
 * `apps/desktop/src/DesktopWorkspace.tsx`, и ровно с той же рабочей областью.
 * Backend приходит из тех же команд Tauri поверх `NativeBackend`, что и на
 * рабочем столе; различие — жест выбора папки: системного диалога на Android
 * нет, и обзор папок рисует само окно (`FolderPicker`).
 *
 * Обзор живёт здесь, рядом с `Workspace`, а не внутри него: выбор папки —
 * платформенный жест, и общая рабочая область о нём знает ровно столько,
 * сколько сказано в `VaultOpener` — «дай путь» (001 T115).
 *
 * Доски здесь такие же, как везде: с 2026-09-23 сборка под Android получает
 * профиль `ReadWrite`, и холст открывается с инструментами (фича 002, FR-033).
 * Право писать доски всё так же приезжает из `VaultClient.boardWriteProfile` —
 * оболочка его не назначает и не подменяет.
 */
export function MobileWorkspace(): React.JSX.Element {
  const picker = useFolderPicker();
  const openVaultFolder = useCallback(() => openMobileVault(picker.pick), [picker.pick]);
  // Та же панель настроек, что на web и рабочем столе; «добавить папку» идёт
  // через тот же обзор, что и открытие vault. `picker.pick` стабилен, поэтому
  // клиент один на всё время жизни окна.
  const device = useMemo(() => tauriDeviceSettings(picker.pick), [picker.pick]);
  // Создание vault из удалённого репозитория (фича 011): пустую папку выбирает
  // тот же обзор, что и папку vault.
  const cloner = useMemo(() => tauriVaultCloner(picker.pick, openVaultAt), [picker.pick]);

  // Откуда начать обзор: последний открытый vault (FR-007). Спрашивается на
  // монтировании, а не при открытии обзора, — чтобы список был готов к первому
  // кадру, а не появлялся после него.
  const [startAt, setStartAt] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    void lastMobileFolder().then((path) => {
      if (!cancelled) setStartAt(path);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <DeviceI18nProvider device={device}>
      <Workspace
        openVaultFolder={openVaultFolder}
        // То же предложение продолжить с последним vault, что на рабочем столе
        // и на web, и из того же хранилища настроек устройства (FR-007).
        lastVault={lastTauriVault}
        cloneVault={cloner}
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
      {picker.isOpen && (
        <FolderPicker startAt={startAt} onChoose={picker.choose} onCancel={picker.cancel} />
      )}
    </DeviceI18nProvider>
  );
}
