import { LANGUAGES, useI18n } from "../i18n/index.js";
import { primarySubtag } from "../i18n/resolve.js";
import type { LanguageChoice, Settings, Theme } from "../wasm-core/typsidian_core.js";
import { EditorFontSizeField } from "./EditorFontSizeField.js";

export interface AppearanceSectionProps {
  settings: Settings;
  onChangeTheme: (theme: Theme) => void;
  onChangeFontSize: (size: number) => void;
  /** Выбор языка интерфейса (фича 010, FR-010). Уходит в настройки устройства. */
  onChangeLanguage: (choice: LanguageChoice) => void;
  /**
   * «Предлагать метки и заметки при наборе» (фича 018, FR-009): привычка
   * устройства. Не передано — выключателя нет.
   */
  onChangeCompletionAuto?: (auto: boolean) => void;
  /** Сочетание ручного вызова — в подписи выключателя; пусто — не назначено. */
  completionShortcut?: string;
  /** Причина отказа из ядра (FR-016) — UI её показывает, но не придумывает. */
  rejectionMessage: string | null;

  /**
   * Можно ли объявить текущие значения умолчаниями устройства (фича 006,
   * FR-031). `false` — vault не открыт: перекрывать нечего, и правка и так
   * уходит прямо в устройство.
   */
  canMakeDefault?: boolean;
  onMakeDefault?: () => void;
  /** Итог последнего такого действия, словами ядра. */
  defaultsMessage?: string | null;
}

/** Значение пункта «как в системе» в списке: не код языка ни при каком выборе. */
const SYSTEM = "system";

/**
 * Раздел «Внешний вид» (T040, FR-015, FR-016).
 *
 * Кнопки «применить» здесь нет намеренно: изменение вступает в силу в момент
 * выбора (FR-003). Правила ввода размера живут в самом поле
 * ([`EditorFontSizeField`]) — тем же полем настраивается размер внутри папки
 * заметок, и двум копиям этих правил ничто не помешало бы разойтись.
 */
export function AppearanceSection({
  settings,
  onChangeTheme,
  onChangeFontSize,
  onChangeLanguage,
  onChangeCompletionAuto,
  completionShortcut = "",
  canMakeDefault = false,
  onMakeDefault,
  defaultsMessage = null,
  rejectionMessage,
}: AppearanceSectionProps): React.JSX.Element {
  const { t } = useI18n();
  // Язык, которого эта версия не знает, показывается как «как в системе» — так
  // он и действует (FR-018). Само значение в файле не трогается, пока
  // пользователь не выберет язык явно.
  const chosen = settings.language === undefined ? undefined : primarySubtag(settings.language);
  const selected = LANGUAGES.some((language) => language.code === chosen) ? chosen : SYSTEM;

  return (
    <section aria-label={t("settings.section.appearance")}>
      <h2>{t("settings.section.appearance")}</h2>

      <label>
        {t("settings.appearance.language")}
        <select
          value={selected}
          onChange={(event) => {
            const value = event.target.value;
            onChangeLanguage(value === SYSTEM ? "system" : { tag: value });
          }}
        >
          <option value={SYSTEM}>{t("settings.appearance.languageSystem")}</option>
          {/* Каждый язык назван на самом себе (FR-011): человек, случайно
              переключившийся на незнакомый язык, найдёт свой по написанию. */}
          {LANGUAGES.map((language) => (
            <option key={language.code} value={language.code} lang={language.code}>
              {language.nativeName}
            </option>
          ))}
        </select>
      </label>
      <p className="ts-field__hint">{t("settings.appearance.languageHint")}</p>

      <label>
        {t("settings.appearance.theme")}
        <select
          value={settings.theme ?? "light"}
          onChange={(event) => onChangeTheme(event.target.value as Theme)}
        >
          <option value="light">{t("settings.appearance.themeLight")}</option>
          <option value="dark">{t("settings.appearance.themeDark")}</option>
        </select>
      </label>

      <EditorFontSizeField
        size={settings.editor_font_size ?? null}
        onChange={onChangeFontSize}
      />

      {onChangeCompletionAuto !== undefined && (
        <>
          <label>
            <input
              type="checkbox"
              checked={settings.editor_completion_auto ?? true}
              onChange={(event) => onChangeCompletionAuto(event.target.checked)}
            />
            {t("completion.setting.auto")}
          </label>
          <p className="ts-field__hint">
            {t("completion.setting.autoHint", { shortcut: completionShortcut })}
          </p>
        </>
      )}

      {rejectionMessage && <p role="alert">{rejectionMessage}</p>}

      {canMakeDefault && onMakeDefault && (
        <>
          {/* FR-029: правка при открытом vault уходит в сам vault. Значит
              умолчания устройства меняются только этим явным действием — без
              него они не менялись бы никогда. */}
          <p>{t("settings.appearance.vaultOwned")}</p>
          <button type="button" onClick={onMakeDefault}>
            {t("settings.appearance.makeDefault")}
          </button>
          {defaultsMessage && <p role="status">{defaultsMessage}</p>}
        </>
      )}
    </section>
  );
}
