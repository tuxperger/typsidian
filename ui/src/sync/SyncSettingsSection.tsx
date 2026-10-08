"use client";

import { useEffect, useState } from "react";
import type {
  SyncError,
  VaultSyncSettingsPatch,
} from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import { describeSyncError } from "./describe.js";
import { asSyncError } from "./sync-client.js";
import type { SyncControl } from "./useSync.js";

export interface SyncSettingsSectionProps {
  sync: SyncControl;
}

/**
 * Настройки синхронизации (FR-007, FR-019, FR-031, FR-037).
 *
 * Два хранилища, и оба — не здесь. Автоснимки и автообмен лежат в настройках
 * vault и доставляются вместе с ним; автор и посредник принадлежат устройству.
 * Границы значений (пауза 30 с…сутки, интервал от минуты) стережёт ядро: поле
 * отправляет введённое и показывает то, что ядро сохранило.
 */
export function SyncSettingsSection({ sync }: SyncSettingsSectionProps): React.JSX.Element {
  const { t } = useI18n();
  const { client, overview } = sync;
  const settings = overview?.settings ?? null;
  const [failure, setFailure] = useState<SyncError | null>(null);
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [proxy, setProxy] = useState("");
  const [deviceLoaded, setDeviceLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const device = await client.deviceSync();
        if (cancelled) return;
        setName(device.author?.name ?? "");
        setEmail(device.author?.email ?? "");
        setProxy(device.cors_proxy ?? "");
        setDeviceLoaded(true);
      } catch (cause) {
        if (!cancelled) setFailure(asSyncError(cause));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  const guarded = async (action: () => Promise<unknown>): Promise<void> => {
    setFailure(null);
    try {
      await action();
      await sync.refresh();
    } catch (cause) {
      setFailure(asSyncError(cause));
    }
  };

  const update = (patch: VaultSyncSettingsPatch): Promise<void> =>
    guarded(() => client.updateSettings(patch));

  const saveAuthor = (): Promise<void> =>
    guarded(() =>
      client.setAuthor(
        name.trim() === "" && email.trim() === ""
          ? null
          : { name: name.trim(), email: email.trim() },
      ),
    );

  const saveProxy = (): Promise<void> =>
    guarded(() => client.setCorsProxy(proxy.trim() === "" ? null : proxy.trim()));

  const isKeeper = overview?.keeper_state.kind === "ThisDevice";
  const hasRemote = settings?.remote_url !== undefined;
  const interval = settings?.auto_pull_interval_secs;

  return (
    <section className="ts-sync-settings" aria-label={t("sync.settings.title")}>
      <h3>{t("sync.settings.auto")}</h3>
      {settings === null ? (
        <p role="status">{t("common.loading")}</p>
      ) : (
        <>
          <label>
            <input
              type="checkbox"
              className="ts-switch"
              checked={settings.auto_snapshot ?? false}
              onChange={(event) => void update({ auto_snapshot: event.target.checked })}
            />
            {t("sync.settings.autoSnapshot")}
          </label>
          <p className="ts-field__hint">
            {isKeeper ? t("sync.settings.autoSnapshotHint") : t("sync.settings.autoSnapshotNotKeeper")}
          </p>

          <label>
            {t("sync.settings.pause")}
            <input
              type="number"
              min={30}
              max={86400}
              step={30}
              // `key` — чтобы поле показало значение, которое сохранило ядро,
              // если оно поправило введённое.
              key={settings.auto_snapshot_pause_secs ?? 300}
              defaultValue={settings.auto_snapshot_pause_secs ?? 300}
              onBlur={(event) => {
                const value = Number(event.target.value);
                if (Number.isFinite(value)) void update({ auto_snapshot_pause_secs: value });
              }}
            />
          </label>

          <label>
            <input
              type="checkbox"
              className="ts-switch"
              checked={settings.auto_push ?? false}
              disabled={!hasRemote}
              onChange={(event) => void update({ auto_push: event.target.checked })}
            />
            {t("sync.settings.autoPush")}
          </label>

          <label>
            <input
              type="checkbox"
              className="ts-switch"
              checked={settings.auto_pull_on_open ?? false}
              disabled={!hasRemote}
              onChange={(event) => void update({ auto_pull_on_open: event.target.checked })}
            />
            {t("sync.settings.autoPullOnOpen")}
          </label>

          <label>
            {t("sync.settings.autoPullInterval")}
            <input
              type="number"
              min={0}
              step={1}
              disabled={!hasRemote}
              key={interval ?? 0}
              defaultValue={interval === undefined ? 0 : Math.round(interval / 60)}
              onBlur={(event) => {
                const minutes = Number(event.target.value);
                if (Number.isFinite(minutes)) {
                  // Ноль — «по интервалу не получать»: у ядра это отсутствие значения.
                  void update({ auto_pull_interval_secs: minutes <= 0 ? 0 : minutes * 60 });
                }
              }}
            />
          </label>
          <p className="ts-field__hint">
            {hasRemote ? t("sync.settings.autoPullIntervalHint") : t("sync.settings.needsRemote")}
          </p>
        </>
      )}

      <h3>{t("sync.settings.author")}</h3>
      <p className="ts-field__hint">{t("sync.settings.authorHint")}</p>
      <label>
        {t("sync.author.name")}
        <input
          value={name}
          disabled={!deviceLoaded}
          autoComplete="off"
          onChange={(event) => setName(event.target.value)}
          onBlur={() => void saveAuthor()}
        />
      </label>
      <label>
        {t("sync.author.email")}
        <input
          type="email"
          value={email}
          disabled={!deviceLoaded}
          autoComplete="off"
          onChange={(event) => setEmail(event.target.value)}
          onBlur={() => void saveAuthor()}
        />
      </label>

      {/* Посредник — только там, где оболочка не обменивается с сервером
          напрямую (FR-037). */}
      {overview !== null && !overview.capabilities.direct_remote && (
        <>
          <h3>{t("sync.remote.proxy")}</h3>
          <p className="ts-field__hint">{t("sync.remote.proxyHint")}</p>
          <label>
            {t("sync.remote.proxy")}
            <input
              value={proxy}
              disabled={!deviceLoaded}
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setProxy(event.target.value)}
              onBlur={() => void saveProxy()}
            />
          </label>
        </>
      )}

      {failure !== null && <p role="alert">{describeSyncError(t, failure)}</p>}
    </section>
  );
}
