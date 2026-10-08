"use client";

import { useI18n } from "../i18n/index.js";
import { describeSyncError, indicatorText } from "./describe.js";
import type { SyncControl } from "./useSync.js";

export interface SyncIndicatorProps {
  sync: SyncControl;
  /** Панель открыта — кнопка показывает это нажатым состоянием. */
  panelOpen: boolean;
  onOpenPanel: () => void;
}

/**
 * Индикатор синхронизации (FR-035, FR-019): одно краткое состояние и вход в
 * панель. Отказ фонового обмена показывается здесь, а не окном поверх работы.
 *
 * Что именно показать, решает `indicatorText` по сводке ядра; своих состояний
 * у индикатора нет.
 */
export function SyncIndicator({
  sync,
  panelOpen,
  onOpenPanel,
}: SyncIndicatorProps): React.JSX.Element {
  const { t } = useI18n();
  const { label, tone } = indicatorText(t, sync.overview, sync.activity, sync.error);
  const detail = sync.error === null ? t("sync.indicator.open") : describeSyncError(t, sync.error);

  return (
    <button
      type="button"
      className="ts-sync-pill"
      data-tone={tone}
      aria-pressed={panelOpen}
      title={detail}
      aria-label={t("sync.indicator.label", { state: label })}
      onClick={onOpenPanel}
    >
      {tone === "busy" ? (
        <span className="ts-hazard" aria-hidden="true">
          <span className="ts-hazard__fill" />
        </span>
      ) : (
        <span className="ts-sync-pill__dot" aria-hidden="true" />
      )}
      {label}
    </button>
  );
}
