import { useEffect, useState } from "react";
import type { Settings, Theme } from "../wasm-core/typsidian_core.js";

/**
 * Границы, объявленные ядром (research.md §11). Дублируются здесь **только**
 * как подсказка полю ввода (`min`/`max`): решение о допустимости принимает
 * ядро, и UI не отклоняет значение сам — он показывает причину, вернувшуюся из
 * `apply` (принцип I, FR-016).
 */
export const FONT_SIZE_HINT = { min: 8, max: 32 } as const;

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
 * выбора (FR-003). Размер шрифта — единственное место, где пользователь может
 * ввести негодное значение, поэтому черновик поля живёт локально до `blur` или
 * `Enter`: иначе набор «20» проходил бы через промежуточное «2», которое ядро
 * честно отвергало бы как выход за диапазон.
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
  const [draft, setDraft] = useState(String(settings.editor_font_size ?? ""));

  // Значение могло измениться снаружи (правка файла руками, другое окно) —
  // поле следует за источником истины, а не за собственным прошлым состоянием.
  useEffect(() => {
    setDraft(String(settings.editor_font_size ?? ""));
  }, [settings.editor_font_size]);

  const commitDraft = (): void => {
    const parsed = Number(draft);
    if (!Number.isInteger(parsed)) {
      setDraft(String(settings.editor_font_size ?? ""));
      return;
    }
    onChangeFontSize(parsed);
  };

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

      <label>
        Размер шрифта редактора
        <input
          type="number"
          inputMode="numeric"
          min={FONT_SIZE_HINT.min}
          max={FONT_SIZE_HINT.max}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={commitDraft}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              commitDraft();
            }
          }}
        />
      </label>

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
