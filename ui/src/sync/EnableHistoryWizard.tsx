"use client";

import { useEffect, useState } from "react";
import type { IgnoreReport, SyncError } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { describeSyncError } from "./describe.js";
import { asSyncError } from "./sync-client.js";
import type { SyncControl } from "./useSync.js";

export interface EnableHistoryWizardProps {
  sync: SyncControl;
  onClose: () => void;
}

type Step = "loading" | "stignore" | "gitignore" | "author" | "running";

/**
 * Мастер включения истории (FR-001, FR-003, FR-007, FR-028).
 *
 * Порядок шагов — требование, а не оформление: правила Syncthing идут первыми,
 * потому что история, появившаяся в папке раньше них, уехала бы на другие
 * устройства. Сам порядок стережёт ядро (`StIgnoreNotConfirmed`); мастер лишь
 * ведёт по нему и показывает точный текст правил до записи.
 */
export function EnableHistoryWizard({
  sync,
  onClose,
}: EnableHistoryWizardProps): React.JSX.Element {
  const { t } = useI18n();
  const { client } = sync;
  const [step, setStep] = useState<Step>("loading");
  const [reports, setReports] = useState<IgnoreReport[]>([]);
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [failure, setFailure] = useState<SyncError | null>(null);
  const [working, setWorking] = useState(false);

  const running = step === "running";
  // Escape во время первого снимка отменяет его, а не закрывает мастер поверх
  // идущей операции (принцип X).
  useDismissOnEscape(running ? sync.cancel : onClose);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [found, device] = await Promise.all([client.ignoreReports(), client.deviceSync()]);
        if (cancelled) return;
        setReports(found);
        setName(device.author?.name ?? "");
        setEmail(device.author?.email ?? "");
        const stignore = found.find((report) => report.file === "StIgnore");
        setStep(stignore !== undefined && stignore.missing.length > 0 ? "stignore" : "gitignore");
      } catch (cause) {
        if (cancelled) return;
        setFailure(asSyncError(cause));
        setStep("gitignore");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  const stignore = reports.find((report) => report.file === "StIgnore") ?? null;
  const gitignore = reports.find((report) => report.file === "GitIgnore") ?? null;

  const guarded = async (action: () => Promise<void>): Promise<void> => {
    setWorking(true);
    setFailure(null);
    try {
      await action();
    } catch (cause) {
      setFailure(asSyncError(cause));
    } finally {
      setWorking(false);
    }
  };

  const acceptStignore = (): Promise<void> =>
    guarded(async () => {
      await client.applyIgnore("StIgnore");
      setStep("gitignore");
    });

  const declineStignore = (): Promise<void> =>
    guarded(async () => {
      await client.declineStignore();
      setStep("gitignore");
    });

  const enable = async (): Promise<void> => {
    setFailure(null);
    setStep("running");
    try {
      const trimmedName = name.trim();
      const trimmedEmail = email.trim();
      // Пустые поля — «спросить общую настройку git»; хватит ли её, решает
      // ядро и при нехватке ответит `AuthorRequired`.
      await client.setAuthor(
        trimmedName === "" && trimmedEmail === ""
          ? null
          : { name: trimmedName, email: trimmedEmail },
      );
      await client.enableHistory(t("sync.wizard.firstMessage"));
      await sync.refresh();
      onClose();
    } catch (cause) {
      const error = asSyncError(cause);
      // Отмена первого снимка — не отказ: мастер остаётся на шаге автора, а
      // история уже включена и ждёт снимка из панели.
      setFailure(error.kind === "Cancelled" ? null : error);
      await sync.refresh();
      if (error.kind === "Cancelled") onClose();
      else setStep("author");
    }
  };

  return (
    <div
      className="ts-dialog ts-sync-wizard"
      role="dialog"
      aria-modal="true"
      aria-label={t("sync.wizard.title")}
    >
      <div className="ts-dialog__head">
        <span className="ts-eyebrow">{t("sync.wizard.title")}</span>
      </div>

      <div className="ts-dialog__body">
        {step === "loading" && <p role="status">{t("common.loading")}</p>}

        {step === "stignore" && stignore !== null && (
          <>
            <p>{t("sync.wizard.stignoreIntro")}</p>
            <pre className="ts-sync-rules" aria-label={t("sync.wizard.rulesLabel")}>
              {stignore.proposed_block}
            </pre>
            <p className="ts-dialog__hint">
              {t("sync.wizard.stignoreNote", { count: stignore.user_lines })}
            </p>
            <p className="ts-dialog__warn">{t("sync.wizard.stignoreDeclineWarning")}</p>
          </>
        )}

        {step === "gitignore" && (
          <>
            <p>{t("sync.wizard.gitignoreIntro")}</p>
            {gitignore !== null && gitignore.missing.length > 0 ? (
              <>
                <pre className="ts-sync-rules" aria-label={t("sync.wizard.rulesLabel")}>
                  {gitignore.proposed_block}
                </pre>
                <p className="ts-dialog__hint">
                  {t("sync.wizard.gitignoreNote", { count: gitignore.user_lines })}
                </p>
              </>
            ) : (
              <p className="ts-dialog__hint">{t("sync.wizard.gitignoreComplete")}</p>
            )}
          </>
        )}

        {(step === "author" || step === "running") && (
          <>
            <p>{t("sync.wizard.authorIntro")}</p>
            <label className="ts-field">
              <span className="ts-field__label">{t("sync.author.name")}</span>
              <input
                className="ts-field__input"
                value={name}
                disabled={running}
                autoComplete="off"
                onChange={(event) => setName(event.target.value)}
              />
            </label>
            <label className="ts-field">
              <span className="ts-field__label">{t("sync.author.email")}</span>
              <input
                className="ts-field__input"
                type="email"
                value={email}
                disabled={running}
                autoComplete="off"
                onChange={(event) => setEmail(event.target.value)}
              />
            </label>
            <p className="ts-dialog__hint">{t("sync.wizard.authorHint")}</p>
            {running && (
              <p role="status">
                <span className="ts-hazard" aria-hidden="true">
                  <span className="ts-hazard__fill" />
                </span>{" "}
                {t("sync.wizard.running")}
              </p>
            )}
          </>
        )}

        {failure !== null && (
          <p className="ts-dialog__error" role="alert">
            {describeSyncError(t, failure)}
          </p>
        )}
      </div>

      <div className="ts-dialog__foot">
        <button type="button" className="ts-btn" onClick={running ? sync.cancel : onClose}>
          {running ? t("sync.cancelOperation") : t("common.cancel")}
        </button>
        <span className="ts-spacer" />
        {step === "stignore" && (
          <>
            <button
              type="button"
              className="ts-btn"
              disabled={working}
              onClick={() => void declineStignore()}
            >
              {t("sync.wizard.stignoreDecline")}
            </button>
            <button
              type="button"
              className="ts-btn ts-btn--accent"
              disabled={working}
              onClick={() => void acceptStignore()}
            >
              {t("sync.wizard.stignoreAccept")}
            </button>
          </>
        )}
        {step === "gitignore" && (
          <button type="button" className="ts-btn ts-btn--accent" onClick={() => setStep("author")}>
            {t("sync.wizard.next")}
          </button>
        )}
        {step === "author" && (
          <button type="button" className="ts-btn ts-btn--accent" onClick={() => void enable()}>
            {t("sync.wizard.enable")}
          </button>
        )}
      </div>
    </div>
  );
}
