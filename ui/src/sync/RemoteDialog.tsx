"use client";

import { useEffect, useRef, useState } from "react";
import type { SyncError } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { describeSyncError } from "./describe.js";
import type { CloneRequest, RemoteCapabilities } from "./sync-client.js";

export interface RemoteDialogProps {
  /** Подключение к открытому vault или создание vault из репозитория. */
  mode: "connect" | "clone";
  capabilities: RemoteCapabilities;
  /** Посредник, уже указанный для этого vault (только web). */
  initialProxy?: string | null;
  /** Адрес, уже записанный в настройках vault, — чтобы подтвердить его или сменить. */
  initialUrl?: string | null;
  /** Операция идёт: поля заперты, «Отмена» прерывает её. */
  busy: boolean;
  error: SyncError | null;
  onSubmit: (request: CloneRequest) => void;
  onCancel: () => void;
}

/**
 * Адрес и данные доступа удалённого репозитория (FR-013, FR-014, FR-014a,
 * FR-037).
 *
 * Проверок здесь нет: годен ли адрес, поддержана ли схема и куда убрать токен,
 * вписанный в адрес, решает ядро при подключении — диалог показывает его
 * ответ. Что оболочка не умеет, не прячется, а объясняется (FR-036).
 */
export function RemoteDialog({
  mode,
  capabilities,
  initialProxy = null,
  initialUrl = null,
  busy,
  error,
  onSubmit,
  onCancel,
}: RemoteDialogProps): React.JSX.Element {
  const { t } = useI18n();
  const [url, setUrl] = useState(initialUrl ?? "");
  const [user, setUser] = useState("");
  const [token, setToken] = useState("");
  const [proxy, setProxy] = useState(initialProxy ?? "");
  const urlRef = useRef<HTMLInputElement | null>(null);

  useDismissOnEscape(onCancel);
  useEffect(() => {
    urlRef.current?.focus();
  }, []);

  const title = mode === "clone" ? t("sync.remote.cloneTitle") : t("sync.remote.connectTitle");
  const ready = url.trim() !== "" && !busy;

  const submit = (): void => {
    if (!ready) return;
    onSubmit({
      url: url.trim(),
      credentials: {
        user: user.trim() === "" ? undefined : user.trim(),
        token: token === "" ? undefined : token,
      },
      corsProxy: proxy.trim() === "" ? null : proxy.trim(),
    });
  };

  return (
    <div className="ts-dialog" role="dialog" aria-modal="true" aria-label={title}>
      <div className="ts-dialog__head">
        <span className="ts-eyebrow">{title}</span>
      </div>

      <form
        className="ts-dialog__body"
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        {mode === "clone" && <p className="ts-dialog__hint">{t("sync.remote.cloneHint")}</p>}

        <label className="ts-field">
          <span className="ts-field__label">{t("sync.remote.url")}</span>
          <input
            ref={urlRef}
            className="ts-field__input ts-field__input--code"
            value={url}
            disabled={busy}
            placeholder="https://example.org/user/notes.git" // i18n-ignore: образец адреса
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => setUrl(event.target.value)}
          />
          <span className="ts-field__hint">
            {capabilities.ssh ? t("sync.remote.urlHintSsh") : t("sync.remote.urlHintNoSsh")}
          </span>
        </label>

        <label className="ts-field">
          <span className="ts-field__label">{t("sync.remote.user")}</span>
          <input
            className="ts-field__input"
            value={user}
            disabled={busy}
            autoComplete="off"
            onChange={(event) => setUser(event.target.value)}
          />
          <span className="ts-field__hint">{t("sync.remote.userHint")}</span>
        </label>

        <label className="ts-field">
          <span className="ts-field__label">{t("sync.remote.token")}</span>
          <input
            className="ts-field__input"
            type="password"
            value={token}
            disabled={busy}
            autoComplete="off"
            onChange={(event) => setToken(event.target.value)}
          />
          <span className="ts-field__hint">
            {capabilities.secureSecrets
              ? t("sync.remote.tokenHintSecure")
              : t("sync.remote.tokenHintBrowser")}
          </span>
        </label>

        {/* Посредник нужен только там, где оболочка не обменивается с сервером
            напрямую (FR-037); в остальных поле не показывается — указывать
            там нечего. */}
        {!capabilities.directRemote && (
          <label className="ts-field">
            <span className="ts-field__label">{t("sync.remote.proxy")}</span>
            <input
              className="ts-field__input ts-field__input--code"
              value={proxy}
              disabled={busy}
              placeholder="https://" // i18n-ignore: начало адреса
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setProxy(event.target.value)}
            />
            <span className="ts-field__hint">{t("sync.remote.proxyHint")}</span>
          </label>
        )}

        {error !== null && (
          <p className="ts-dialog__error" role="alert">
            {describeSyncError(t, error)}
            {error.kind === "CorsBlocked" && ` ${t("sync.remote.corsAdvice")}`}
          </p>
        )}
        {busy && (
          <p className="ts-dialog__hint" role="status">
            <span className="ts-hazard" aria-hidden="true">
              <span className="ts-hazard__fill" />
            </span>{" "}
            {mode === "clone" ? t("sync.remote.cloning") : t("sync.remote.connecting")}
          </p>
        )}
        {/* Enter в любом поле отправляет форму; видимая кнопка — в подвале. */}
        <button type="submit" hidden />
      </form>

      <div className="ts-dialog__foot">
        <button type="button" className="ts-btn" onClick={onCancel}>
          {busy ? t("sync.cancelOperation") : t("common.cancel")}
        </button>
        <span className="ts-spacer" />
        <button type="button" className="ts-btn ts-btn--accent" disabled={!ready} onClick={submit}>
          {mode === "clone" ? t("sync.remote.clone") : t("sync.remote.connect")}
        </button>
      </div>
    </div>
  );
}
