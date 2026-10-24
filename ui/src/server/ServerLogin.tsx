"use client";

import { useState } from "react";
import { useI18n } from "../i18n/index.js";
import {
  login,
  oidcStartUrl,
  type ServerFailure,
  type ServerInfo,
  type ServerSession,
} from "./server-api.js";

// Экран входа (фича 037, макет S2): логин и пароль, кнопка провайдера, если он
// настроен. Регистрации нет — аккаунты заводит администратор (US6 AS4). Неверный
// вход не уточняет, что именно неверно (US1 AS2).

/** Почему показан экран входа, если не по первому открытию. */
export type LoginNotice = "expired" | "unknownAccount" | "oidcFailed";

export interface ServerLoginProps {
  info: ServerInfo;
  notice?: LoginNotice;
  /** Логин уже известен — повторный вход после истёкшей сессии. */
  initialLogin?: string;
  onSignedIn: (session: ServerSession) => void;
  /** «← Папка на устройстве»; нет — ссылки нет (повторный вход поверх работы). */
  onBack?: () => void;
}

export function ServerLogin({
  info,
  notice,
  initialLogin = "",
  onSignedIn,
  onBack,
}: ServerLoginProps): React.JSX.Element {
  const { t } = useI18n();
  const [loginName, setLoginName] = useState(initialLogin);
  const [password, setPassword] = useState("");
  const [failure, setFailure] = useState<ServerFailure | null>(null);
  const [busy, setBusy] = useState(false);
  const host = typeof location === "undefined" ? "" : location.host;

  const submit = async (): Promise<void> => {
    setBusy(true);
    const result = await login(loginName.trim(), password);
    setBusy(false);
    if (result.ok) {
      setPassword("");
      onSignedIn(result.value);
    } else {
      setFailure(result.failure);
    }
  };

  const message = ((): React.ReactNode => {
    if (failure !== null) {
      switch (failure.kind) {
        case "BadCredentials":
          return <div className="ts-server-alert" role="alert">{t("server.login.bad")}</div>;
        case "TooManyAttempts":
          return (
            <div className="ts-server-alert ts-server-alert--warn" role="alert">
              <b>{t("server.login.tooManyTitle")}</b>{" "}
              {t("server.login.tooMany", {
                login: loginName.trim(),
                minutes: Math.max(1, Math.ceil(failure.retry_after_s / 60)),
              })}
            </div>
          );
        case "Offline":
          return <div className="ts-server-alert" role="alert">{t("server.login.offline")}</div>;
        default:
          return <div className="ts-server-alert" role="alert">{t("server.login.failed")}</div>;
      }
    }
    switch (notice) {
      case "expired":
        return (
          <div className="ts-server-alert ts-server-alert--warn" role="status">
            {t("server.login.expired")}
          </div>
        );
      case "unknownAccount":
        return (
          <div className="ts-server-alert" role="alert">
            {t("server.login.unknownAccount", { label: info.oidc ?? "" })}
          </div>
        );
      case "oidcFailed":
        return <div className="ts-server-alert" role="alert">{t("server.login.oidcFailed")}</div>;
      default:
        return null;
    }
  })();

  return (
    <form
      className="ts-server-card"
      onSubmit={(event) => {
        event.preventDefault();
        if (!busy) void submit();
      }}
    >
      <h1 className="ts-server-card__brand">
        <span className="ts-wordmark__slash">/</span>typsidian
      </h1>
      <p className="ts-server-card__host">{host}</p>
      {info.password && (
        <>
          <label className="ts-field">
            <span className="ts-field__label">{t("server.login.login")}</span>
            <input
              className="ts-field__input"
              name="username"
              autoComplete="username"
              autoCapitalize="none"
              spellCheck={false}
              required
              value={loginName}
              onChange={(event) => setLoginName(event.target.value)}
            />
          </label>
          <label className="ts-field">
            <span className="ts-field__label">{t("server.login.password")}</span>
            <input
              className="ts-field__input"
              type="password"
              name="password"
              autoComplete="current-password"
              required
              value={password}
              onChange={(event) => setPassword(event.target.value)}
            />
          </label>
        </>
      )}
      {message}
      {info.password && (
        <button
          type="submit"
          className="ts-btn ts-btn--accent ts-server-card__wide"
          disabled={busy}
        >
          {t("server.login.submit")}
        </button>
      )}
      {info.oidc !== undefined && (
        <>
          {info.password && <div className="ts-server-or">{t("server.login.or")}</div>}
          <a className="ts-btn ts-btn--outline ts-server-card__wide" href={oidcStartUrl()}>
            {t("server.login.oidc", { label: info.oidc })}
          </a>
        </>
      )}
      {onBack !== undefined && (
        <button type="button" className="ts-btn ts-btn--quiet" onClick={onBack}>
          {t("server.login.back")}
        </button>
      )}
    </form>
  );
}
