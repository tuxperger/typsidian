"use client";

import { useEffect, useState } from "react";
import type { VaultRecord } from "../wasm-core/typsidian_core.js";
import type { DeviceSettingsClient } from "../settings/device-settings-client.js";
import { describeOpenVaultError } from "../vault-tree/OpenVault.js";
import type { OpenVaultError } from "../vault-tree/useVault.js";
import { useI18n } from "../i18n/index.js";
import { Icon } from "../icons/Icons.js";
import { vaultDisplayName } from "./knownVaults.js";

/**
 * Стартовый экран (фича 043, FR-008; экран 2 Typsidian.pdf): слева знак,
 * объяснение и способы начать, справа недавние vault.
 *
 * Последний vault здесь не «продолжается» отдельной кнопкой: он первым стоит в
 * списке недавних и выделен, а открывается тем же нажатием, что и остальные.
 * Недоступная папка остаётся в списке с причиной — приложение её не разыскивает
 * (004 FR-023), и «Указать заново» добавляет её выбором папки.
 */
export function WelcomeScreen({
  isLoading,
  error,
  device,
  onOpenFolder,
  onOpenKnown,
  onClone,
  onSettings,
  extra,
}: {
  isLoading: boolean;
  error: OpenVaultError | null;
  device: DeviceSettingsClient | undefined;
  onOpenFolder: () => void;
  /** Открыть известный vault; не передан — список без перехода. */
  onOpenKnown: ((id: string) => void) | undefined;
  onClone: (() => void) | undefined;
  onSettings: (() => void) | undefined;
  extra?: React.ReactNode;
}): React.JSX.Element {
  const { t } = useI18n();
  const message = error === null ? null : describeOpenVaultError(t, error);
  const [recent, setRecent] = useState<{ records: VaultRecord[]; current: string | undefined }>({
    records: [],
    current: undefined,
  });

  useEffect(() => {
    if (device === undefined) return undefined;
    let cancelled = false;
    device
      .snapshot()
      .then((snapshot) => {
        if (cancelled) return;
        const current = snapshot.settings.current_vault;
        // Последний открытый — первым: он и есть «продолжить».
        const records = [...snapshot.knownVaults].sort(
          (a, b) => Number(b.id === current) - Number(a.id === current),
        );
        setRecent({ records, current });
      })
      .catch(() => {
        // Хранилище устройства недоступно — недавних не показать, открыть
        // папку можно по-прежнему.
      });
    return () => {
      cancelled = true;
    };
  }, [device]);

  return (
    <div className="ts-start">
      <div className="ts-start__intro">
        <h1 className="ts-start__mark">
          <span className="ts-wordmark__slash">/</span>typsidian
        </h1>
        <p className="ts-start__lead">{t("workspace.start.lead")}</p>
        <p className="ts-start__about">{t("workspace.start.about")}</p>
        <div className="ts-start__actions">
          <button type="button" className="ts-btn ts-btn--accent ts-start__action" disabled={isLoading} onClick={onOpenFolder}>
            <Icon name="folder" size={14} />
            {isLoading ? t("tree.open.opening") : t("workspace.start.openFolder")}
          </button>
          {onClone !== undefined && (
            <button type="button" className="ts-btn ts-btn--outline ts-start__action" disabled={isLoading} onClick={onClone}>
              <Icon name="download" size={14} />
              {t("sync.command.clone")}
            </button>
          )}
        </div>
        {message !== null && (
          <p className="ts-start__error" role="alert">
            {message}
          </p>
        )}
        {onSettings !== undefined && (
          <button type="button" className="ts-start__link" onClick={onSettings}>
            <Icon name="sliders" size={13} />
            {t("workspace.start.settings")}
          </button>
        )}
        {extra}
      </div>

      {recent.records.length > 0 && (
        <section className="ts-start__recent" aria-labelledby="ts-start-recent">
          <h2 id="ts-start-recent" className="ts-eyebrow">
            {t("workspace.start.recent")}
          </h2>
          <ul className="ts-start__list">
            {recent.records.map((record) => {
              const unavailable = record.availability === "unavailable";
              const last = record.id === recent.current;
              return (
                <li key={record.id} className="ts-start__item" data-last={last ? "" : undefined}>
                  <button
                    type="button"
                    className="ts-start__open"
                    disabled={isLoading || unavailable || onOpenKnown === undefined}
                    onClick={() => onOpenKnown?.(record.id)}
                  >
                    <Icon name="folder" size={14} />
                    <span className="ts-start__names">
                      <span className="ts-start__name">{vaultDisplayName(record)}</span>
                      <span className="ts-start__location">{record.location}</span>
                      {unavailable && (
                        <span className="ts-start__problem">{t("workspace.start.unavailable")}</span>
                      )}
                    </span>
                  </button>
                  {unavailable && (
                    <button type="button" className="ts-btn ts-btn--outline" disabled={isLoading} onClick={onOpenFolder}>
                      {t("workspace.start.relink")}
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      )}
    </div>
  );
}
