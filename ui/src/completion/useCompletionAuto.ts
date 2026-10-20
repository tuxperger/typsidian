"use client";

import { useEffect, useState } from "react";
import type { DeviceSettingsClient } from "../settings/device-settings-client.js";

/**
 * «Предлагать метки и заметки при наборе» (фича 018, FR-009) — значение
 * устройства. Перечитывается, когда закрывается панель настроек: выключатель
 * живёт там, а не здесь. Нет хранилища — умолчание ядра, `true`.
 */
export function useCompletionAuto(
  device: DeviceSettingsClient | undefined,
  settingsShown: boolean,
): boolean {
  const [auto, setAuto] = useState(true);
  useEffect(() => {
    if (device === undefined || settingsShown) return undefined;
    let alive = true;
    void device
      .snapshot()
      .then((snapshot) => {
        if (alive) setAuto(snapshot.settings.editor_completion_auto ?? true);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [device, settingsShown]);
  return auto;
}
