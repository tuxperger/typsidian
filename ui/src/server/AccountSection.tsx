"use client";

import { useCallback, useEffect, useState } from "react";
import { useI18n, type I18n } from "../i18n/index.js";
import type { ServerConnection } from "./connection.js";
import {
  changePassword,
  endSession,
  logoutAll,
  sessions as listSessions,
  type ServerSessionEntry,
} from "./server-api.js";
import { agoText } from "./when.js";

// Настройки → «Аккаунт» (фича 037, макет S4, US5): смена пароля и входы аккаунта —
// завершить любой или выйти на всех устройствах.

export interface AccountSectionProps {
  connection: ServerConnection;
  /** Текущая сессия завершена («Выйти на всех устройствах»). */
  onSignedOut: () => void;
}

/** «Firefox, Linux» из строки браузера — только для подписи, без точности. */
function browserOf(i18n: I18n, agent: string): string {
  const browser = /Edg\//.test(agent)
    ? "Edge"
    : /Firefox\//.test(agent)
      ? "Firefox"
      : /Chrome\//.test(agent)
        ? "Chrome"
        : /Safari\//.test(agent)
          ? "Safari"
          : i18n.t("server.account.unknownBrowser");
  const system = /Android/.test(agent)
    ? "Android"
    : /iPhone|iPad/.test(agent)
      ? "iOS"
      : /Mac OS X/.test(agent)
        ? "macOS"
        : /Windows/.test(agent)
          ? "Windows"
          : /Linux/.test(agent)
            ? "Linux"
            : "";
  return system === "" ? browser : `${browser}, ${system}`;
}

export function AccountSection({ connection, onSignedOut }: AccountSectionProps): React.JSX.Element {
  const i18n = useI18n();
  const { t } = i18n;
  const session = connection.session();
  const [list, setList] = useState<ServerSessionEntry[]>([]);
  const [editing, setEditing] = useState(false);
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [repeat, setRepeat] = useState("");
  const [message, setMessage] = useState<{ text: string; bad: boolean } | null>(null);
  const host = typeof location === "undefined" ? "" : location.host;

  const reload = useCallback(async () => {
    const result = await listSessions();
    if (result.ok) setList(result.value);
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const savePassword = async (): Promise<void> => {
    if ([...next].length < 12) return setMessage({ text: t("server.account.tooShort"), bad: true });
    if (next !== repeat) return setMessage({ text: t("server.account.mismatch"), bad: true });
    const result = await changePassword(connection.session(), current, next);
    if (result.ok) {
      setEditing(false);
      setCurrent("");
      setNext("");
      setRepeat("");
      setMessage({ text: t("server.account.changed"), bad: false });
      void reload();
    } else if (result.failure.kind === "Invalid" && result.failure.reason === "Wrong") {
      setMessage({ text: t("server.account.wrong"), bad: true });
    } else if (result.failure.kind === "Invalid" && result.failure.reason === "TooShort") {
      setMessage({ text: t("server.account.tooShort"), bad: true });
    } else {
      setMessage({ text: t("server.vaults.failed"), bad: true });
    }
  };

  return (
    <section aria-label={t("server.account.section")} className="ts-server-account">
      <div className="ts-settings__head">
        <h2>{t("server.account.section")}</h2>
      </div>
      <div className="ts-server-account__row">
        <span>
          <b>{session.login}</b>
          <span className="ts-server-list__meta">
            {session.hasPassword
              ? t("server.account.via", { host })
              : t("server.account.viaOidc", { host })}
          </span>
        </span>
        {session.hasPassword && !editing && (
          <button type="button" className="ts-btn ts-btn--outline" onClick={() => setEditing(true)}>
            {t("server.account.changePassword")}
          </button>
        )}
      </div>
      {editing && (
        <form
          className="ts-server-account__password"
          onSubmit={(event) => {
            event.preventDefault();
            void savePassword();
          }}
        >
          {(
            [
              ["server.account.current", current, setCurrent, "current-password"],
              ["server.account.next", next, setNext, "new-password"],
              ["server.account.repeat", repeat, setRepeat, "new-password"],
            ] as const
          ).map(([label, value, set, autoComplete]) => (
            <label key={label} className="ts-field">
              <span className="ts-field__label">{t(label)}</span>
              <input
                className="ts-field__input"
                type="password"
                autoComplete={autoComplete}
                value={value}
                onChange={(event) => set(event.target.value)}
              />
            </label>
          ))}
          <div className="ts-server-row">
            <button type="submit" className="ts-btn ts-btn--accent">
              {t("server.account.save")}
            </button>
            <button type="button" className="ts-btn ts-btn--quiet" onClick={() => setEditing(false)}>
              {t("server.vaults.cancel")}
            </button>
          </div>
        </form>
      )}
      {message !== null && (
        <div
          className={message.bad ? "ts-server-alert" : "ts-server-alert ts-server-alert--ok"}
          role={message.bad ? "alert" : "status"}
        >
          {message.text}
        </div>
      )}

      <h3 className="ts-server-account__sub">{t("server.account.sessions")}</h3>
      <ul className="ts-server-sessions">
        {list.map((entry) => (
          <li key={entry.id} className="ts-server-sessions__row">
            <span>
              <b>{browserOf(i18n, entry.userAgent)}</b>
              {entry.current && <span className="ts-server-sessions__current">{t("server.account.thisSession")}</span>}
              <span className="ts-server-list__meta">
                {t("server.account.lastSeen", { when: agoText(i18n, entry.lastSeen), ip: entry.ip })}
              </span>
            </span>
            {!entry.current && (
              <button
                type="button"
                className="ts-btn ts-btn--outline"
                onClick={() => void endSession(connection.session(), entry.id).then(() => reload())}
              >
                {t("server.account.end")}
              </button>
            )}
          </li>
        ))}
      </ul>
      <div>
        <button
          type="button"
          className="ts-btn ts-btn--danger"
          onClick={() =>
            void logoutAll(connection.session()).then((result) => {
              if (result.ok) onSignedOut();
            })
          }
        >
          {t("server.account.logoutAll")}
        </button>
      </div>
    </section>
  );
}
