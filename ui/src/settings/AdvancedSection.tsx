import { useState } from "react";
import type {
  Diagnostics,
  OutageReason,
  StoreCapabilities,
} from "../wasm-core/typsidian_core.js";
import { useI18n, type Translate } from "../i18n/index.js";

export interface AdvancedSectionProps {
  capabilities: StoreCapabilities | null;
  /** Диагностика из ядра (FR-039); `null`, пока хранилище не открыто. */
  diagnostics: Diagnostics | null;
  onReveal: () => void;
  onResetToDefaults: () => void;
  /** Итог последнего сброса словами — отказ объясняется, а не замалчивается. */
  resetMessage?: string | null;
}

/**
 * Раздел «Дополнительно» (T041, T095, FR-036, FR-037, FR-039).
 *
 * Расположение хранилища показывается всегда, а действие раскрытия — **только**
 * при `can_reveal`. Ветвление идёт по значению, пришедшему из ядра, а не по
 * таргету: в вебе раскрывать нечего, и там же, где на native кнопка, стоит
 * пояснение, почему её нет (research.md §13).
 *
 * Сброс к умолчаниям выполняется в два шага: FR-037 требует явного
 * подтверждения, и оно сделано отдельным состоянием, а не `window.confirm` —
 * диалог браузера невозможно проверить сценарием и он выглядит чужеродно.
 */
export function AdvancedSection({
  capabilities,
  diagnostics,
  onReveal,
  onResetToDefaults,
  resetMessage = null,
}: AdvancedSectionProps): React.JSX.Element {
  const { t } = useI18n();
  const [isConfirmingReset, setIsConfirmingReset] = useState(false);

  return (
    <section aria-label={t("settings.section.advanced")}>
      <h2>{t("settings.section.advanced")}</h2>

      {capabilities && (
        <p>
          {t("settings.advanced.storage")} <code>{capabilities.location}</code>{" "}
          {capabilities.can_reveal ? (
            <button type="button" onClick={onReveal}>
              {t("settings.advanced.reveal")}
            </button>
          ) : (
            <span role="note">{t("settings.advanced.cannotReveal")}</span>
          )}
        </p>
      )}

      {capabilities && !capabilities.writable && (
        <p role="status">{t("settings.notice.readOnly")}</p>
      )}

      <h3>{t("settings.advanced.resetTitle")}</h3>
      {isConfirmingReset ? (
        <div role="group" aria-label={t("settings.advanced.resetConfirm")}>
          <p role="alert">{t("settings.advanced.resetWarning")}</p>
          <button
            type="button"
            onClick={() => {
              setIsConfirmingReset(false);
              onResetToDefaults();
            }}
          >
            {t("settings.fontSize.clear")}
          </button>
          <button type="button" onClick={() => setIsConfirmingReset(false)}>
            {t("common.cancel")}
          </button>
        </div>
      ) : (
        <button type="button" onClick={() => setIsConfirmingReset(true)}>
          {t("settings.advanced.resetButton")}
        </button>
      )}
      {resetMessage && <p role="status">{resetMessage}</p>}

      <h3>{t("settings.advanced.diagnostics")}</h3>
      {diagnostics ? (
        <DiagnosticsView diagnostics={diagnostics} />
      ) : (
        <p role="status">{t("settings.advanced.notOpen")}</p>
      )}
    </section>
  );
}

/**
 * FR-039 и FR-042: показывается ровно то, что собрало ядро, и никуда не
 * отправляется. Кнопки «отправить в поддержку» здесь нет и быть не может —
 * передать сведения может только пользователь, скопировав их руками.
 */
function DiagnosticsView({
  diagnostics,
}: {
  diagnostics: Diagnostics;
}): React.JSX.Element {
  const { t } = useI18n();
  const { app_version, editor_mode, editor_mode_availability, environment, storage } =
    diagnostics;

  return (
    <dl>
      <dt>{t("settings.advanced.version")}</dt>
      <dd>{app_version}</dd>

      <dt>{t("settings.mode.label")}</dt>
      <dd>{describeMode(t, editor_mode)}</dd>

      <dt>{t("settings.mode.full")}</dt>
      <dd>
        {editor_mode_availability.kind === "Available"
          ? t("settings.advanced.available")
          : t("settings.advanced.unavailable", {
              reason: describeOutage(t, editor_mode_availability.reason),
            })}
      </dd>

      <dt>{t("settings.advanced.environment")}</dt>
      <dd>
        {environment.target} · {environment.os_family} ·{" "}
        {environment.can_spawn_processes
          ? t("settings.advanced.processesAvailable")
          : t("settings.advanced.processesUnavailable")}
      </dd>

      <dt>{t("settings.advanced.storageShort")}</dt>
      <dd>
        {storage.capabilities.location} ·{" "}
        {storage.capabilities.writable
          ? t("settings.advanced.writable")
          : t("settings.advanced.readOnly")}
      </dd>
    </dl>
  );
}

function describeMode(t: Translate, mode: Diagnostics["editor_mode"]): string {
  switch (mode.state) {
    case "FullBackend":
      return t("settings.advanced.modeFull");
    case "SimplifiedEmulation":
      return t("settings.advanced.modeSimplified");
    case "PlainInput":
      return t("settings.advanced.modePlain", { reason: describeOutage(t, mode.reason) });
    default: {
      const exhaustive: never = mode;
      return exhaustive;
    }
  }
}

/** Причина отказа называется словами (FR-034), а не кодом варианта. */
function describeOutage(t: Translate, reason: OutageReason): string {
  switch (reason.kind) {
    case "ExternalProcessesUnavailable":
      return t("settings.advanced.outage.noProcesses");
    case "BackendNotInstalled":
      return t("settings.advanced.outage.notInstalled");
    case "BackendStopped":
      return t("settings.advanced.outage.stopped");
    case "ConfigMissingOnThisDevice":
      return t("settings.advanced.outage.configMissing");
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}
