import type { Settings, Theme } from "../wasm-core/typsidian_core.js";
import { EditorFontSizeField } from "./EditorFontSizeField.js";

export interface AppearanceSectionProps {
  settings: Settings;
  onChangeTheme: (theme: Theme) => void;
  onChangeFontSize: (size: number) => void;
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
  canMakeDefault = false,
  onMakeDefault,
  defaultsMessage = null,
  rejectionMessage,
}: AppearanceSectionProps): React.JSX.Element {
  return (
    <section aria-label="Внешний вид">
      <h2>Внешний вид</h2>

      <label>
        Тема
        <select
          value={settings.theme ?? "light"}
          onChange={(event) => onChangeTheme(event.target.value as Theme)}
        >
          <option value="light">Светлая</option>
          <option value="dark">Тёмная</option>
        </select>
      </label>

      <EditorFontSizeField
        size={settings.editor_font_size ?? null}
        onChange={onChangeFontSize}
      />

      {rejectionMessage && <p role="alert">{rejectionMessage}</p>}

      {canMakeDefault && onMakeDefault && (
        <>
          {/* FR-029: правка при открытом vault уходит в сам vault. Значит
              умолчания устройства меняются только этим явным действием — без
              него они не менялись бы никогда. */}
          <p>
            Эти значения принадлежат открытому vault и переезжают вместе с ним.
          </p>
          <button type="button" onClick={onMakeDefault}>
            Сделать значениями по умолчанию
          </button>
          {defaultsMessage && <p role="status">{defaultsMessage}</p>}
        </>
      )}
    </section>
  );
}
