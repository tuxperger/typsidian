"use client";

import { useCallback, useEffect, useState } from "react";
import type { SpellConfig, SpellLanguage } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n, type MessageKey } from "../i18n/index.js";
import { refreshSpelling } from "../editor/highlight/typstHighlight.js";
import type { VaultSettingsState } from "./useVaultSettings.js";

/**
 * Раздел «Орфография» (фича 030, US2, US3, макет O3): переключатель, языки и
 * словарь vault списком с добавлением и «убрать».
 *
 * Правила — ядра: какие слова годятся в словарь, где он лежит и как
 * нормализуется. Здесь — показ и вызовы; после каждой правки открытые
 * редакторы перепроверяются.
 */
export interface SpellcheckSectionProps {
  vaultSettings: VaultSettingsState;
  client: VaultClient | null;
}

const LANGUAGES: { id: SpellLanguage; label: MessageKey }[] = [
  { id: "ru", label: "spellcheck.settings.lang.ru" },
  { id: "en", label: "spellcheck.settings.lang.en" },
];

/** Код отказа ядра (`SpellError`) из исключения клиента. */
function errorKey(cause: unknown): MessageKey {
  const code = typeof cause === "string" ? cause : "";
  if (code === "InvalidWord") return "spellcheck.error.InvalidWord";
  if (code === "ReadOnly") return "spellcheck.error.ReadOnly";
  return "spellcheck.error.Io";
}

export function SpellcheckSection({ vaultSettings, client }: SpellcheckSectionProps): React.JSX.Element {
  const { t } = useI18n();
  const overrides = vaultSettings.overrides;
  const [config, setConfig] = useState<SpellConfig | null>(null);
  const [word, setWord] = useState("");
  const [problem, setProblem] = useState<MessageKey | null>(null);

  const reconfigure = useCallback(async () => {
    if (client === null) return;
    setConfig(await client.spelling.configure());
    refreshSpelling();
  }, [client]);

  // Настройки vault поменялись (здесь или снаружи) — проверяющий перечитывает их.
  useEffect(() => {
    void reconfigure().catch(() => setConfig(null));
  }, [reconfigure, overrides]);

  if (overrides === null) {
    return vaultSettings.error !== null ? (
      <p role="alert">{vaultSettings.error(t)}</p>
    ) : (
      <p role="status">{t("settings.vaultNotOpen")}</p>
    );
  }

  const enabled = overrides.spellcheck_enabled ?? true;
  const languages: string[] = overrides.spellcheck_languages ?? LANGUAGES.map((language) => language.id);

  const toggleLanguage = (id: SpellLanguage) => {
    const next = languages.includes(id)
      ? languages.filter((language) => language !== id)
      : [...languages, id];
    // Пустой список ядро читает как «оба»: последний язык не снимается.
    if (next.length === 0) return;
    void vaultSettings.setSpellcheck({
      enabled,
      languages: next.length === LANGUAGES.length ? undefined : next,
    });
  };

  const change = async (op: "add" | "remove", value: string): Promise<void> => {
    if (client === null) return;
    try {
      const next =
        op === "add" ? await client.spelling.addWord(value) : await client.spelling.removeWord(value);
      setConfig(next);
      setProblem(null);
      if (op === "add") setWord("");
      refreshSpelling();
    } catch (cause) {
      setProblem(errorKey(cause));
    }
  };

  const words = config?.words ?? [];

  return (
    <div className="ts-srows">
      {vaultSettings.error !== null && <p role="alert">{vaultSettings.error(t)}</p>}
      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("spellcheck.settings.enabled")}</span>
          <span className="ts-srow__hint">{t("spellcheck.settings.enabledHint")}</span>
        </span>
        <input
          id="ts-spellcheck-enabled"
          type="checkbox"
          role="switch"
          className="ts-switch"
          checked={enabled}
          onChange={(event) =>
            void vaultSettings.setSpellcheck({
              enabled: event.target.checked,
              languages: overrides.spellcheck_languages ?? undefined,
            })
          }
        />
      </label>
      <div className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("spellcheck.settings.languages")}</span>
          <span className="ts-srow__hint">{t("spellcheck.settings.languagesHint")}</span>
        </span>
        <span className="ts-spell__langs">
          {LANGUAGES.map((language) => (
            <label key={language.id} className="ts-spell__lang">
              <input
                id={`ts-spellcheck-${language.id}`}
                type="checkbox"
                checked={languages.includes(language.id)}
                disabled={!enabled}
                onChange={() => toggleLanguage(language.id)}
              />
              {t(language.label)}
            </label>
          ))}
        </span>
      </div>
      <div className="ts-spell__dict" role="group" aria-label={t("spellcheck.settings.section")}>
        <div className="ts-spell__dhead">
          <span>
            {words.length === 0
              ? t("spellcheck.settings.dictionaryEmpty")
              : t("spellcheck.settings.dictionary", { count: words.length })}
          </span>
          <span className="ts-spacer" />
          <span>{t("spellcheck.settings.file")}</span>
        </div>
        <form
          className="ts-spell__add"
          onSubmit={(event) => {
            event.preventDefault();
            if (word.trim() !== "") void change("add", word);
          }}
        >
          <input
            id="ts-spellcheck-word"
            className="ts-field__input"
            placeholder={t("spellcheck.settings.newWord")}
            aria-label={t("spellcheck.settings.newWord")}
            value={word}
            onChange={(event) => setWord(event.target.value)}
          />
          <button type="submit" className="ts-btn" disabled={client === null || word.trim() === ""}>
            {t("spellcheck.settings.add")}
          </button>
        </form>
        {problem !== null && (
          <p role="alert" className="ts-srow__hint ts-spell__problem">
            {t(problem)}
          </p>
        )}
        {words.length === 0 ? (
          <p className="ts-spell__empty">{t("spellcheck.settings.empty")}</p>
        ) : (
          <ul className="ts-spell__words">
            {words.map((entry) => (
              <li key={entry} className="ts-spell__word">
                <span>{entry}</span>
                <button
                  type="button"
                  className="ts-btn ts-btn--quiet ts-spell__remove"
                  aria-label={t("spellcheck.settings.removeLabel", { word: entry })}
                  onClick={() => void change("remove", entry)}
                >
                  ✕ {t("spellcheck.settings.remove")}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
