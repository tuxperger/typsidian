"use client";

import { useEffect, useId, useState } from "react";
import { useI18n } from "../i18n/index.js";

/**
 * Границы, объявленные ядром (`settings::MIN_FONT_SIZE`/`MAX_FONT_SIZE`,
 * research.md §11). Здесь они — **подсказка полю ввода**, а не второе место,
 * решающее, что допустимо: значение вне диапазона ядро отбрасывает при чтении
 * файла (`vault_settings::parse`) и отклоняет при записи на устройство.
 *
 * `fallback` — с чего начинают стрелки, когда размер не задан вовсе. Зеркало
 * `settings::DEFAULT_FONT_SIZE` и тоже подсказка: первое нажатие «+» обязано
 * дать осмысленное число, а не единицу.
 */
export const FONT_SIZE_HINT = { min: 8, max: 32, fallback: 14 } as const;

export interface EditorFontSizeFieldProps {
  /** Действующий размер в пикселях. `null` — не задан. */
  size: number | null;
  onChange: (size: number) => void;
  /**
   * Снять заданный размер. Не передан — снимать нечего: у хранилища устройства
   * значение есть всегда, «пусто» в нём не выражается.
   */
  onClear?: () => void;
  /** Строка под полем: чем размер станет, если его снять. */
  hint?: string;
}

/**
 * Поле размера шрифта редактора — одно на оба хранилища (T040, FR-015).
 *
 * Общее, а не по копии в каждой панели: правила ввода тут неочевидные, и
 * разойтись двум копиям было бы нечем не заметить. Черновик поля живёт
 * локально до `blur` или `Enter`: иначе набор «20» проходил бы через
 * промежуточное «2», а его пришлось бы либо записать, либо объявить ошибкой.
 *
 * Негодное значение не пишется и не показывается ошибкой, а откатывается к
 * действующему: записать в файл число, которое ядро отбросит при следующем
 * чтении, значило бы соврать пользователю о том, что размер применён.
 */
export function EditorFontSizeField({
  size,
  onChange,
  onClear,
  hint,
}: EditorFontSizeFieldProps): React.JSX.Element {
  const { t } = useI18n();
  const [draft, setDraft] = useState(size === null ? "" : String(size));
  // Подпись связана с полем по id, а не обёрткой `<label>`: между ними стоят
  // стрелки, и обёртка перехватывала бы нажатия на них.
  const inputId = useId();

  // Значение могло измениться снаружи (правка файла руками, другое окно) —
  // поле следует за источником истины, а не за собственным прошлым состоянием.
  useEffect(() => {
    setDraft(size === null ? "" : String(size));
  }, [size]);

  const commit = (value: number): void => {
    if (
      !Number.isInteger(value) ||
      value < FONT_SIZE_HINT.min ||
      value > FONT_SIZE_HINT.max
    ) {
      setDraft(size === null ? "" : String(size));
      return;
    }
    onChange(value);
  };

  // Стрелки считают от действующего размера, а не от черновика: набранное, но
  // не подтверждённое число — ещё не размер.
  const step = (delta: number): void =>
    commit((size ?? FONT_SIZE_HINT.fallback) + delta);

  return (
    <div className="ts-fontsize">
      <label className="ts-fontsize__label" htmlFor={inputId}>
        {t("settings.fontSize.label")}
      </label>

      <div className="ts-stepper">
        <button
          type="button"
          className="ts-btn ts-stepper__btn"
          aria-label={t("settings.fontSize.decrease")}
          disabled={size !== null && size <= FONT_SIZE_HINT.min}
          onClick={() => step(-1)}
        >
          −
        </button>

        <input
          id={inputId}
          className="ts-field__input ts-stepper__value"
          type="number"
          inputMode="numeric"
          min={FONT_SIZE_HINT.min}
          max={FONT_SIZE_HINT.max}
          placeholder={t("settings.fontSize.default")}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={() => commit(Number(draft))}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              commit(Number(draft));
            }
          }}
        />

        <button
          type="button"
          className="ts-btn ts-stepper__btn"
          aria-label={t("settings.fontSize.increase")}
          disabled={size !== null && size >= FONT_SIZE_HINT.max}
          onClick={() => step(1)}
        >
          +
        </button>

        {onClear && (
          <button
            type="button"
            className="ts-btn ts-btn--quiet"
            disabled={size === null}
            onClick={onClear}
          >
            {t("settings.fontSize.clear")}
          </button>
        )}
      </div>

      <p className="ts-fontsize__hint">
        {t("settings.fontSize.range", { min: FONT_SIZE_HINT.min, max: FONT_SIZE_HINT.max })}
        {hint ? ` ${hint}` : ""}
      </p>
    </div>
  );
}
