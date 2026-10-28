"use client";

import { useI18n, type MessageKey } from "../i18n/index.js";
import type { PaneView } from "./usePaneLayout.js";

const VIEWS: readonly { id: PaneView; label: MessageKey }[] = [
  { id: "text", label: "workspace.view.text" },
  { id: "side", label: "workspace.view.side" },
  { id: "document", label: "workspace.view.document" },
];

/**
 * «Текст / Рядом / Документ» в шапке (фича 043, FR-002; экран 1 Typsidian.pdf).
 *
 * Сегменты — кнопки с `aria-pressed`, а не радиогруппа: каждая сразу меняет
 * раскладку, и стрелочная навигация радиогруппы переключала бы вид на каждом
 * шаге фокуса.
 */
export function ViewSwitch({
  value,
  onChange,
  disabled = false,
  disabledHint,
}: {
  value: PaneView;
  onChange: (view: PaneView) => void;
  disabled?: boolean;
  /** Почему переключатель недоступен — подсказкой на всей группе. */
  disabledHint?: string;
}): React.JSX.Element {
  const { t } = useI18n();
  return (
    <div
      className="ts-segmented"
      role="group"
      aria-label={t("workspace.view.label")}
      title={disabled ? disabledHint : undefined}
    >
      {VIEWS.map((view) => (
        <button
          key={view.id}
          type="button"
          className="ts-segmented__item"
          aria-pressed={!disabled && value === view.id}
          disabled={disabled}
          onClick={() => onChange(view.id)}
        >
          {t(view.label)}
        </button>
      ))}
    </div>
  );
}
