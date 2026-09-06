"use client";

import { useEffect, useState } from "react";
import { SettingsPanel } from "@typsidian/ui/settings/SettingsPanel.js";
import type { PreviewClient } from "@typsidian/ui/preview-client.js";
import {
  createWebPreviewClient,
  openWebVault,
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
 */
export function VaultWorkspace(): React.JSX.Element {
  const [preview, setPreview] = useState<PreviewClient | null>(null);

  // Компилятор создаётся лениво и после монтирования: модуль тяжёлый, и
  // держать его загрузку на пути первого рендера незачем — пока его нет,
  // колонка предпросмотра честно говорит, что собирается (005/research.md §6).
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const client = await createWebPreviewClient();
      if (!cancelled) setPreview(client);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <Workspace
      openVaultFolder={openWebVault}
      settings={<SettingsPanel />}
      preview={preview}
    />
  );
}
