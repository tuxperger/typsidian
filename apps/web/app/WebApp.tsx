"use client";

import { useEffect, useState } from "react";
import { DeviceI18nProvider } from "@typsidian/ui/i18n/device.js";
import { useI18n } from "@typsidian/ui/i18n/index.js";
import { serverInfo } from "@typsidian/ui/server/server-api.js";
import { StartScreen, type WebMode } from "@typsidian/ui/server/StartScreen.js";
import { ServerWorkspace } from "./ServerWorkspace";
import { VaultWorkspace } from "./VaultWorkspace";
import { device } from "./web-device";

/** Выбранный способ — удобство этого браузера, не настройка vault. */
const MODE_KEY = "typsidian.webMode";

function savedMode(): WebMode | null {
  try {
    const value = localStorage.getItem(MODE_KEY);
    return value === "device" || value === "server" ? value : null;
  } catch {
    return null;
  }
}

function saveMode(mode: WebMode | null): void {
  try {
    if (mode === null) localStorage.removeItem(MODE_KEY);
    else localStorage.setItem(MODE_KEY, mode);
  } catch {
    // Хранилище недоступно (приватное окно) — выбор спросят снова.
  }
}

/**
 * Web: папка на устройстве или свой сервер (фича 037, FR-001, макет S1). Есть ли
 * сервер, отвечает `GET /api/info` того же адреса; без него стартовый экран
 * предлагает только папку.
 */
export function WebApp(): React.JSX.Element {
  // `undefined` — ещё не прочитано (первая отрисовка на сервере сборки).
  const [mode, setMode] = useState<WebMode | null | undefined>(undefined);
  const [serverAvailable, setServerAvailable] = useState<boolean | null>(null);

  useEffect(() => {
    setMode(savedMode());
    void serverInfo().then((info) => setServerAvailable(info !== null));
  }, []);

  const choose = (next: WebMode | null): void => {
    saveMode(next);
    setMode(next);
  };

  if (mode === undefined) return <main className="ts-server-screen" />;
  if (mode === "server" && serverAvailable !== false) {
    return (
      <DeviceI18nProvider device={device}>
        <ServerWorkspace onDeviceMode={() => choose("device")} />
      </DeviceI18nProvider>
    );
  }
  if (mode === "device") {
    return (
      <VaultWorkspace
        welcomeExtra={
          serverAvailable === true ? <SwitchToServer onSwitch={() => choose("server")} /> : null
        }
      />
    );
  }
  if (serverAvailable === null) return <main className="ts-server-screen" />;
  return (
    <DeviceI18nProvider device={device}>
      <StartScreen serverAvailable={serverAvailable} onChoose={choose} />
    </DeviceI18nProvider>
  );
}

function SwitchToServer({ onSwitch }: { onSwitch: () => void }): React.JSX.Element {
  const { t } = useI18n();
  return (
    <button type="button" className="ts-btn ts-btn--quiet" onClick={onSwitch}>
      {t("server.start.switch")}
    </button>
  );
}
