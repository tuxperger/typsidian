"use client";

import { useCallback, useEffect, useState } from "react";
import type { SnapshotUsage } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";
import type { VaultSettingsState } from "./useVaultSettings.js";

/**
 * Раздел «Снимки заметок» (фича 029, US3, макет S3): срок хранения, интервал
 * снимков при правке, занятое место и «Очистить» с подтверждением.
 *
 * Пределы и умолчания — ядра (`SNAPSHOT_KEEP_DAYS`, `DEFAULT_KEEP_DAYS`):
 * значение вне пределов ядро при чтении считает «не задано».
 */
export interface SnapshotsSectionProps {
  vaultSettings: VaultSettingsState;
  client: VaultClient | null;
}

/** Умолчания ядра (`snapshots::DEFAULT_*`) — для показа пустого поля. */
const DEFAULT_KEEP_DAYS = 7;
const DEFAULT_INTERVAL_MINUTES = 5;

export function SnapshotsSection({ vaultSettings, client }: SnapshotsSectionProps): React.JSX.Element {
  const { t, formatBytes } = useI18n();
  const overrides = vaultSettings.overrides;
  const [usage, setUsage] = useState<SnapshotUsage | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [cleared, setCleared] = useState<number | null>(null);

  const loadUsage = useCallback(async () => {
    if (client === null) return;
    setUsage(await client.snapshots.usage());
  }, [client]);

  useEffect(() => {
    void loadUsage().catch(() => setUsage(null));
  }, [loadUsage]);

  if (overrides === null) {
    return vaultSettings.error !== null ? (
      <p role="alert">{vaultSettings.error(t)}</p>
    ) : (
      <p role="status">{t("settings.vaultNotOpen")}</p>
    );
  }

  const keep = overrides.snapshot_keep_days ?? DEFAULT_KEEP_DAYS;
  const interval = overrides.snapshot_interval_minutes ?? DEFAULT_INTERVAL_MINUTES;

  const save = (next: { keepDays?: number; intervalMinutes?: number }) =>
    void vaultSettings.setSnapshots({
      keepDays: next.keepDays ?? overrides.snapshot_keep_days ?? undefined,
      intervalMinutes: next.intervalMinutes ?? overrides.snapshot_interval_minutes ?? undefined,
    });

  const clear = async (): Promise<void> => {
    if (client === null) return;
    const count = await client.snapshots.clear();
    setConfirming(false);
    setCleared(count);
    await loadUsage();
  };

  return (
    <div className="ts-srows">
      {vaultSettings.error !== null && <p role="alert">{vaultSettings.error(t)}</p>}
      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("snapshots.settings.keep")}</span>
          <span className="ts-srow__hint">{t("snapshots.settings.keepHint")}</span>
        </span>
        <span className="ts-snapshots__num">
          <input
            id="ts-snapshots-keep"
            className="ts-field__input"
            type="number"
            min={1}
            max={365}
            value={keep}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (Number.isInteger(value) && value >= 1 && value <= 365) save({ keepDays: value });
            }}
          />
          {t("snapshots.settings.days")}
        </span>
      </label>
      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("snapshots.settings.interval")}</span>
          <span className="ts-srow__hint">{t("snapshots.settings.intervalHint")}</span>
        </span>
        <span className="ts-snapshots__num">
          <input
            id="ts-snapshots-interval"
            className="ts-field__input"
            type="number"
            min={1}
            max={120}
            value={interval}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (Number.isInteger(value) && value >= 1 && value <= 120) {
                save({ intervalMinutes: value });
              }
            }}
          />
          {t("snapshots.settings.minutes")}
        </span>
      </label>
      <div className="ts-srow">
        <span className="ts-srow__hint">
          {usage !== null &&
            t("snapshots.settings.usage", {
              count: usage.snapshots,
              size: formatBytes(Number(usage.bytes)),
            })}
        </span>
        <button
          type="button"
          className="ts-btn ts-btn--danger"
          disabled={client === null || usage === null || usage.snapshots === 0}
          onClick={() => setConfirming(true)}
        >
          {t("snapshots.settings.clear")}
        </button>
      </div>
      {confirming && usage !== null && (
        <div className="ts-replace__panel ts-replace__panel--warn" role="alertdialog">
          <span className="ts-replace__title">
            {t("snapshots.settings.clearConfirm", { count: usage.snapshots })}
          </span>
          <span className="ts-replace__meta">{t("snapshots.settings.clearHint")}</span>
          <div className="ts-replace__row">
            <button type="button" className="ts-btn ts-btn--danger" onClick={() => void clear()}>
              {t("snapshots.settings.clearDo")}
            </button>
            <button type="button" className="ts-btn ts-btn--quiet" onClick={() => setConfirming(false)}>
              {t("snapshots.cancel")}
            </button>
          </div>
        </div>
      )}
      {cleared !== null && (
        <p role="status" className="ts-srow__hint">
          {t("snapshots.settings.cleared", { count: cleared })}
        </p>
      )}
    </div>
  );
}
