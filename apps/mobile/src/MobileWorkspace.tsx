import { useCallback, useEffect, useState } from "react";
import { VaultSettingsPanel } from "@typsidian/ui/settings/VaultSettingsPanel.js";
import { lastTauriVault } from "@typsidian/ui/vault-client-tauri.js";
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
    <>
      <Workspace
        openVaultFolder={openVaultFolder}
        // То же предложение продолжить с последним vault, что на рабочем столе
        // и на web, и из того же хранилища настроек устройства (FR-007).
        lastVault={lastTauriVault}
        // Та же урезанная панель, что на рабочем столе, и по той же причине:
        // `SettingsPanel` работает через `WebStore` (OPFS + IndexedDB), а эта
        // сборка wasm не грузит вовсе. Само хранилище устройства ей доступно —
        // последний vault приходит именно оттуда, — но через команды оболочки,
        // а не через объект ядра, которого в окне нет.
        settings={(client) => <VaultSettingsPanel client={client} />}
      />
      {picker.isOpen && (
        <FolderPicker startAt={startAt} onChoose={picker.choose} onCancel={picker.cancel} />
      )}
    </>
  );
}
