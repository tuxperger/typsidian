"use client";

import { useEffect, useState } from "react";
import type {
  Capture,
  CaptureOverrides,
  CaptureSettings,
  DailyName,
  EntryStyle,
  Template,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { deviceToday } from "../capture/useDailyNote.js";
import { useI18n } from "../i18n/index.js";
import { NOTHING_OVERRIDDEN } from "../workspace/useShortcuts.js";
import type { VaultSettingsState } from "./useVaultSettings.js";

/**
 * Раздел «Захват и дневник» (фича 022, US3, макет D6): заметка входящих, папка
 * и шаблон ежедневных заметок, вид названия по дате, вид записи захвата.
 *
 * Хранится вместе с vault (006) и одинаков на всех устройствах. Решений здесь
 * нет (принцип I): умолчания, годность путей и нормальную форму даёт ядро,
 * образец записи под настройкой пересчитывает оно же теми же правилами, что и
 * настоящий захват (FR-011).
 */
export interface CaptureSectionProps {
  vaultSettings: VaultSettingsState;
  client: VaultClient | null;
}

const NAMES: readonly DailyName[] = ["YYYY-MM-DD", "YYYY/MM/YYYY-MM-DD", "DD.MM.YYYY"];

/** Ключи текстовых полей, которые ядро читает как «пустое — не задано». */
type TextKey = "inbox" | "daily_folder" | "daily_template";

export function CaptureSection({ vaultSettings, client }: CaptureSectionProps): React.JSX.Element {
  const { t, lang } = useI18n();
  const overrides = vaultSettings.overrides;
  const [settings, setSettings] = useState<CaptureSettings | null>(null);
  const [sample, setSample] = useState("");
  const [todayPath, setTodayPath] = useState<string | null>(null);
  const [templates, setTemplates] = useState<Template[]>([]);
  // Черновики полей путей: пока поле правят, показывается набранное.
  const [drafts, setDrafts] = useState<Partial<Record<TextKey, string>>>({});

  // Действующие настройки и производное от них считает ядро.
  useEffect(() => {
    if (client === null || overrides === null) return undefined;
    let alive = true;
    void (async () => {
      try {
        const effective = await client.capture.settings(overrides);
        const example: Capture = {
          id: "sample",
          content: {
            kind: "Link",
            url: t("capture.settings.sampleUrl"),
            title: t("capture.settings.sampleTitle"),
          },
          note: t("capture.settings.sampleNote"),
          at: { ...deviceToday(), hour: 14, minute: 7 },
          target: { kind: "Inbox" },
        };
        const [line, path] = await Promise.all([
          client.capture.entry(effective, example),
          client.capture.dailyPath(effective, deviceToday()),
        ]);
        if (!alive) return;
        setSettings(effective);
        setSample(line);
        setTodayPath(path ?? null);
      } catch {
        // Образец не посчитался — поля остаются на прежних значениях.
      }
    })();
    return () => {
      alive = false;
    };
    // Язык меняет текст образца.
  }, [client, overrides, t, lang]);

  useEffect(() => {
    if (client === null) return undefined;
    let alive = true;
    void client
      .templateLibrary()
      .then((view) => {
        if (alive) setTemplates(view.templates);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [client]);

  if (overrides === null || settings === null) {
    return vaultSettings.error !== null ? (
      <p role="alert">{vaultSettings.error(t)}</p>
    ) : (
      <p role="status">{t("capture.settings.openVault")}</p>
    );
  }

  /** Записать одно поле: остальные ключи таблицы остаются как были. */
  const patch = async (change: Partial<CaptureOverrides>): Promise<boolean> => {
    const next: CaptureOverrides = { ...(overrides.capture ?? NOTHING_CAPTURE), ...change };
    return vaultSettings.setCapture(next);
  };

  const commitText = async (key: TextKey): Promise<void> => {
    const draft = drafts[key];
    if (draft === undefined) return;
    const value = draft.trim();
    const { [key]: _removed, ...rest } = overrides.capture ?? NOTHING_CAPTURE;
    void _removed;
    const ok = await vaultSettings.setCapture(value === "" ? rest : { ...rest, [key]: value });
    // Отказ показывает раздел словами ядра; черновик остаётся, чтобы его поправить.
    if (ok) setDrafts(({ [key]: _done, ...others }) => others);
  };

  const textField = (key: TextKey, current: string, label: string): React.JSX.Element => (
    <input
      type="text"
      className="ts-field__input ts-capture-settings__input"
      value={drafts[key] ?? current}
      aria-label={label}
      spellCheck={false}
      onChange={(event) => setDrafts({ ...drafts, [key]: event.target.value })}
      onBlur={() => void commitText(key)}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          void commitText(key);
        }
        if (event.key === "Escape") {
          setDrafts(({ [key]: _cancelled, ...others }) => others);
        }
      }}
    />
  );

  const templatePaths = templates.map((entry) => entry.path);
  const chosenTemplate = settings.daily_template ?? "";
  return (
    <div className="ts-srows ts-capture-settings">
      {vaultSettings.error !== null && <p role="alert">{vaultSettings.error(t)}</p>}

      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("capture.settings.inbox")}</span>
          <span className="ts-srow__hint">{t("capture.settings.inboxHint")}</span>
        </span>
        {textField("inbox", settings.inbox, t("capture.settings.inbox"))}
      </label>

      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("capture.settings.dailyFolder")}</span>
        </span>
        {textField("daily_folder", settings.daily_folder, t("capture.settings.dailyFolder"))}
      </label>

      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("capture.settings.dailyTemplate")}</span>
          <span className="ts-srow__hint">{t("capture.settings.dailyTemplateHint")}</span>
        </span>
        <select
          className="ts-field__input ts-capture-settings__input"
          value={chosenTemplate}
          onChange={(event) => {
            const value = event.target.value;
            const { daily_template: _removed, ...rest } = overrides.capture ?? NOTHING_CAPTURE;
            void _removed;
            void vaultSettings.setCapture(value === "" ? rest : { ...rest, daily_template: value });
          }}
        >
          <option value="">{t("capture.settings.noTemplate")}</option>
          {chosenTemplate !== "" && !templatePaths.includes(chosenTemplate) && (
            <option value={chosenTemplate}>{chosenTemplate}</option>
          )}
          {templates.map((entry) => (
            <option key={entry.path} value={entry.path}>
              {entry.path}
            </option>
          ))}
        </select>
      </label>

      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("capture.settings.dailyName")}</span>
          {todayPath !== null && (
            <span className="ts-srow__hint">
              {t("capture.settings.dailyNameToday", { path: todayPath })}
            </span>
          )}
        </span>
        <select
          className="ts-field__input ts-capture-settings__input"
          value={settings.daily_name}
          onChange={(event) => void patch({ daily_name: event.target.value as DailyName })}
        >
          {NAMES.map((name) => (
            <option key={name} value={name}>
              {t(`capture.settings.name.${name}`)}
            </option>
          ))}
        </select>
      </label>

      <div className="ts-srow ts-srow--flat ts-capture-settings__style">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("capture.settings.entryStyle")}</span>
        </span>
        <div className="ts-capture-settings__segs" role="group" aria-label={t("capture.settings.entryStyle")}>
          {(["list", "paragraph"] as const satisfies readonly EntryStyle[]).map((style) => (
            <button
              key={style}
              type="button"
              className="ts-capture__seg"
              aria-pressed={settings.entry_style === style}
              onClick={() => void patch({ entry_style: style })}
            >
              {t(`capture.settings.style.${style}`)}
            </button>
          ))}
          <button
            type="button"
            className="ts-capture__seg"
            aria-pressed={settings.entry_time}
            onClick={() => void patch({ entry_time: !settings.entry_time })}
          >
            {t("capture.settings.style.time")}
          </button>
        </div>
      </div>

      <div className="ts-capture-settings__sample">
        <span className="ts-srow__hint">{t("capture.settings.sample")}</span>
        <pre className="ts-capture-settings__line" aria-label={t("capture.settings.sample")}>
          {sample.trimEnd()}
        </pre>
      </div>

      <p className="ts-srow__hint">{t("capture.settings.footer")}</p>
    </div>
  );
}

/** Таблица без заданных ключей: всё действует по умолчанию. */
const NOTHING_CAPTURE: CaptureOverrides = {};
