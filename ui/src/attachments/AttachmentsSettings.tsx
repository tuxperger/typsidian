"use client";

import { useState } from "react";
import { useI18n } from "../i18n/index.js";
import type { AttachmentsPlacement } from "../wasm-core/typsidian_core.js";

/** Название папки по умолчанию — содержимое vault, одно для всех языков. */
const DEFAULT_FOLDER = "attachments";

export interface AttachmentsSettingsProps {
  /** Размещение с умолчанием. */
  placement: AttachmentsPlacement;
  /** Название папки с умолчанием. */
  folder: string;
  /** Записать; `false` — ядро отклонило название (FR-018). */
  onChange: (next: {
    placement: AttachmentsPlacement | undefined;
    folder: string | undefined;
  }) => Promise<boolean>;
  /** Причина последнего отказа словами ядра. */
  error: string | null;
}

/**
 * Раздел «Вложения» настроек vault (фича 016, макет 1c): где лежат новые
 * вложения и как называется папка. Годность названия проверяет ядро при
 * записи; отказ показывается под полем, прежнее значение остаётся.
 */
export function AttachmentsSettings({
  placement,
  folder,
  onChange,
  error,
}: AttachmentsSettingsProps): React.JSX.Element {
  const { t } = useI18n();
  // Черновик пишется по Enter или уходу из поля, а не на каждое нажатие.
  const [draft, setDraft] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const shown = draft ?? folder;

  const commit = async (next: {
    placement: AttachmentsPlacement;
    folder: string;
  }): Promise<void> => {
    const value = next.folder.trim();
    const ok = await onChange({
      // Умолчания уходят из файла ключами, а не значениями.
      placement: next.placement === "next-to-note" ? undefined : next.placement,
      folder: value === DEFAULT_FOLDER ? undefined : value,
    });
    setFailed(!ok);
    if (ok) setDraft(null);
  };

  const options: ReadonlyArray<{
    value: AttachmentsPlacement;
    title: "settings.attachments.nextToNote" | "settings.attachments.vaultFolder";
    hint: "settings.attachments.nextToNoteHint" | "settings.attachments.vaultFolderHint";
    example: string;
  }> = [
    {
      value: "next-to-note",
      title: "settings.attachments.nextToNote",
      hint: "settings.attachments.nextToNoteHint",
      example: `${t("settings.attachments.exampleFolder")}/${shown}/${t("settings.attachments.exampleFile")}`,
    },
    {
      value: "vault-folder",
      title: "settings.attachments.vaultFolder",
      hint: "settings.attachments.vaultFolderHint",
      example: `${shown}/${t("settings.attachments.exampleFile")}`,
    },
  ];

  return (
    <fieldset className="ts-attach-settings">
      <legend>{t("settings.attachments.title")}</legend>
      <p className="ts-attach-settings__intro">{t("settings.attachments.intro")}</p>
      {options.map((option) => (
        <label key={option.value} className="ts-attach-settings__option">
          <input
            type="radio"
            name="attachments-placement"
            value={option.value}
            checked={placement === option.value}
            onChange={() => void commit({ placement: option.value, folder: shown })}
          />
          <span className="ts-attach-settings__title">{t(option.title)}</span>
          <span className="ts-attach-settings__hint">
            {t(option.hint)} <code>{option.example}</code>
          </span>
        </label>
      ))}
      <label className="ts-attach-settings__field">
        <span className="ts-attach-settings__title">{t("settings.attachments.folder")}</span>
        <input
          type="text"
          value={shown}
          aria-label={t("settings.attachments.folder")}
          aria-invalid={failed && error !== null}
          spellCheck={false}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={() => {
            if (draft !== null) void commit({ placement, folder: draft });
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && draft !== null) {
              event.preventDefault();
              void commit({ placement, folder: draft });
            }
            if (event.key === "Escape") {
              setDraft(null);
              setFailed(false);
            }
          }}
        />
      </label>
      {failed && error !== null && (
        <p role="alert" className="ts-attach-settings__error">
          {error}
        </p>
      )}
      <p className="ts-attach-settings__owned">{t("settings.attachments.owned")}</p>
    </fieldset>
  );
}
