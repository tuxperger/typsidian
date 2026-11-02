"use client";

import { useEffect, useState } from "react";
import type { DeviceSettingsClient } from "../settings/device-settings-client.js";

/**
 * Id записи открытого vault на устройстве — по нему ядро находит ключ API
 * (`AiSection` берёт то же `current_vault`). Читается заново, когда открывается другой vault (`vault`): запись о нём появляется после открытия. Пустая строка: хранилища устройства
 * нет, и ключ искать негде; на сервере id подставляет сервер.
 */
export function useVaultId(device: DeviceSettingsClient | undefined, vault: unknown): string {
  const [id, setId] = useState("");
  useEffect(() => {
    if (device === undefined) return;
    let alive = true;
    void device
      .snapshot()
      .then((snapshot) => alive && setId(snapshot.settings.current_vault ?? ""))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [device, vault]);
  return id;
}
