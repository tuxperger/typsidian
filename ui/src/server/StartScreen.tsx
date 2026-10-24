"use client";

import { useState } from "react";
import { useI18n } from "../i18n/index.js";

// Стартовый экран web (фича 037, макет S1, FR-001): где лежат заметки — в папке на
// устройстве (как раньше) или на своём сервере.

export type WebMode = "device" | "server";

export interface StartScreenProps {
  /** Отвечает ли по этому адресу сервер Typsidian; `null` — ещё не известно. */
  serverAvailable: boolean | null;
  onChoose: (mode: WebMode) => void;
}

export function StartScreen({ serverAvailable, onChoose }: StartScreenProps): React.JSX.Element {
  const { t } = useI18n();
  const [mode, setMode] = useState<WebMode>(serverAvailable === true ? "server" : "device");
  const host = typeof location === "undefined" ? "" : location.host;
  const choice = (value: WebMode, icon: string, title: string, hint: string, disabled = false) => (
    <label
      className={
        "ts-server-choice" +
        (mode === value ? " ts-server-choice--on" : "") +
        (disabled ? " ts-server-choice--off" : "")
      }
    >
      <input
        type="radio"
        name="ts-web-mode"
        value={value}
        checked={mode === value}
        disabled={disabled}
        onChange={() => setMode(value)}
      />
      <span className="ts-server-choice__icon" aria-hidden="true">
        {icon}
      </span>
      <span>
        <b>{title}</b>
        <span>{hint}</span>
      </span>
    </label>
  );
  return (
    <main className="ts-server-screen">
      <form
        className="ts-server-card"
        onSubmit={(event) => {
          event.preventDefault();
          onChoose(mode);
        }}
      >
        <h1 className="ts-server-card__brand">
          <span className="ts-wordmark__slash">/</span>typsidian
        </h1>
        <p className="ts-server-card__lead">{t("server.start.title")}</p>
        {choice("device", "▣", t("server.start.device"), t("server.start.deviceHint"))}
        {choice(
          "server",
          "⇄",
          t("server.start.server"),
          serverAvailable === false
            ? t("server.start.serverUnavailable")
            : t("server.start.serverHint", { host }),
          serverAvailable !== true,
        )}
        <button type="submit" className="ts-btn ts-btn--accent ts-server-card__wide">
          {t("server.start.continue")}
        </button>
      </form>
    </main>
  );
}
