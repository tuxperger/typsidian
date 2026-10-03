import type {
  ConfigDiskState,
  EditorMode,
  OutageReason,
} from "../wasm-core/typsidian_core.js";
import { useI18n, type Translate } from "../i18n/index.js";

export interface KeybindingsSectionProps {
  mode: EditorMode | null;
  configDiskState: ConfigDiskState | null;
  /** Путь к указанному конфигу, если он выбран (FR-026). */
  configPath: string | null;
  /** Доступен ли полноценный режим на этом устройстве (FR-034). */
  fullBackendUnavailableReason: OutageReason | null;
  onChooseFullBackend: () => void;
  onChooseSimplifiedEmulation: () => void;
  onPickConfig: () => void;
  onApplyConfig: () => void;
  onLeavePlainInput: () => void;
}

/**
 * Раздел «Кейбиндинги» (T082, FR-025…FR-035).
 *
 * `EditorMode` — трёхвариантный размеченный union, а не булев флаг: пропуск
 * `PlainInput` в разборе обязан быть ошибкой компиляции (FR-025, принцип IV),
 * потому что именно это состояние легче всего забыть — оно наступает само.
 *
 * Полноценный режим предлагается только когда он доступен. Если недоступен —
 * на месте кнопки стоит причина: упрощённая эмуляция не выдаётся за
 * полноценный режим (FR-034).
 */
export function KeybindingsSection({
  mode,
  configDiskState,
  configPath,
  fullBackendUnavailableReason,
  onChooseFullBackend,
  onChooseSimplifiedEmulation,
  onPickConfig,
  onApplyConfig,
  onLeavePlainInput,
}: KeybindingsSectionProps): React.JSX.Element {
  const { t } = useI18n();
  return (
    <section aria-label={t("settings.section.keybindings")}>
      <h2>{t("settings.section.keybindings")}</h2>

      <p role="status">{describeMode(t, mode)}</p>

      {/* FR-035: переход в обычный ввод — заметное сообщение с причиной и
          действием возврата, а не тихая смена поведения редактора. */}
      {mode?.state === "PlainInput" && (
        <div role="alert">
          <p>{describeOutage(t, mode.reason)}</p>
          <button type="button" onClick={onLeavePlainInput}>
            {t("settings.mode.backToEditing")}
          </button>
        </div>
      )}

      <div role="group" aria-label={t("settings.mode.label")}>
        {fullBackendUnavailableReason ? (
          <p role="note">
            {t("settings.mode.fullUnavailable", {
              reason: describeOutage(t, fullBackendUnavailableReason),
            })}
          </p>
        ) : (
          <button type="button" onClick={onChooseFullBackend}>
            {t("settings.mode.full")}
          </button>
        )}
        <button type="button" onClick={onChooseSimplifiedEmulation}>
          {t("settings.mode.simplified")}
        </button>
      </div>

      <div role="group" aria-label={t("settings.config.label")}>
        <p>
          {t("settings.config.prefix")}{" "}
          {configPath ? <code>{configPath}</code> : <em>{t("settings.config.none")}</em>}{" "}
          <button type="button" onClick={onPickConfig}>
            {configPath ? t("settings.config.chooseOther") : t("settings.config.choose")}
          </button>
        </p>
        {configDiskState && (
          <ConfigStatus
            diskState={configDiskState}
            hasPath={configPath !== null}
            onApplyConfig={onApplyConfig}
          />
        )}
      </div>
    </section>
  );
}

/**
 * Признак расхождения конфига с применённым (FR-026). Применение — только по
 * явному действию: автоматически применять нельзя, поэтому «Применить» — это
 * кнопка, а не побочный эффект отрисовки.
 */
function ConfigStatus({
  diskState,
  hasPath,
  onApplyConfig,
}: {
  diskState: ConfigDiskState;
  hasPath: boolean;
  onApplyConfig: () => void;
}): React.JSX.Element | null {
  const { t } = useI18n();
  switch (diskState) {
    case "inSync":
      return <span role="note">{t("settings.config.applied")}</span>;
    case "changed":
      return (
        <>
          <span role="note">{t("settings.config.changed")}</span>
          <button type="button" onClick={onApplyConfig}>
            {t("insert.form.apply")}
          </button>
        </>
      );
    case "missing":
      if (!hasPath) return null;
      return (
        <span role="note">{t("settings.config.missing")}</span>
      );
    case "unreadable":
      return (
        <span role="note">{t("settings.config.unreadable")}</span>
      );
    default: {
      const exhaustive: never = diskState;
      return exhaustive;
    }
  }
}

function describeMode(t: Translate, mode: EditorMode | null): string {
  if (!mode) return t("settings.mode.unknown");
  switch (mode.state) {
    case "FullBackend":
      return t("settings.mode.fullActive");
    case "SimplifiedEmulation":
      return t("settings.mode.simplifiedActive");
    case "PlainInput":
      // Формулировка намеренно не смешивается с эмуляцией: отсутствие
      // vim-раскладки должно быть однозначно отличимо от штатного режима.
      return t("settings.mode.plainInput");
    default: {
      const exhaustive: never = mode;
      return exhaustive;
    }
  }
}

function describeOutage(t: Translate, reason: OutageReason): string {
  switch (reason.kind) {
    case "ExternalProcessesUnavailable":
      return t("settings.outage.noProcesses");
    case "BackendNotInstalled":
      return t("settings.outage.notInstalled");
    case "BackendStopped":
      return t("settings.outage.stopped");
    case "ConfigMissingOnThisDevice":
      // Фича 006, FR-027: путь приехал вместе с папкой заметок с другой
      // машины. Чинится правкой пути, а не установкой подсистемы.
      return t("settings.outage.configMissing");
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}
