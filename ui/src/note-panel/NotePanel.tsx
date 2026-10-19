"use client";

import { useEffect, useRef } from "react";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import type { NotePanelModeSpec, PanelModeId } from "./modes.js";

// Оболочка колонки заметки (фича 013, макет «Typsidian Связи» 1a и 1c).
//
// Режимов она не знает: получает реестр и рисует шапку с названиями и
// счётчиком активного режима и его тело. Режимы добавляют 015 и 017, не меняя
// этот файл (contracts/note-panel.md).

export interface NotePanelProps {
  modes: readonly NotePanelModeSpec[];
  active: PanelModeId;
  onSelect: (mode: PanelModeId) => void;
  /**
   * `column` — колонка между деревом и редактором; `sheet` — шторка снизу на
   * скриме для узкого окна. Решает ширина окна, а не таргет.
   */
  presentation: "column" | "sheet";
  /** Закрытие шторки: Escape, тап на скрим и ручку. У колонки не используется. */
  onClose: () => void;
}

export function NotePanel({
  modes,
  active,
  onSelect,
  presentation,
  onClose,
}: NotePanelProps): React.JSX.Element {
  const { t } = useI18n();
  const sheet = presentation === "sheet";
  const spec = modes.find((mode) => mode.id === active) ?? modes[0];

  // Escape закрывает только шторку: колонка — часть рабочей области, а не
  // наложение.
  useDismissOnEscape(sheet ? onClose : null);

  // Шторка модальна для скринридера: фокус уходит в неё при открытии.
  const panelRef = useRef<HTMLElement | null>(null);
  useEffect(() => {
    if (sheet) panelRef.current?.focus();
  }, [sheet]);

  // Режимов больше, чем помещается в шапку (017 — пятый): включённый
  // командой или сочетанием режим прокручивается в видимую часть.
  const modesRef = useRef<HTMLDivElement | null>(null);
  const activeId = spec?.id;
  useEffect(() => {
    const strip = modesRef.current;
    const button = strip?.querySelector<HTMLElement>(".ts-notepanel__mode--on");
    if (strip == null || button == null) return;
    const left = button.offsetLeft - strip.offsetLeft;
    if (left < strip.scrollLeft) strip.scrollLeft = left;
    else if (left + button.offsetWidth > strip.scrollLeft + strip.clientWidth) {
      strip.scrollLeft = left + button.offsetWidth - strip.clientWidth;
    }
  }, [activeId]);

  if (spec === undefined) return <></>;

  const panel = (
    <aside
      ref={panelRef}
      className={sheet ? "ts-notepanel ts-notepanel--sheet" : "ts-notepanel"}
      aria-label={t("notePanel.label")}
      {...(sheet ? { role: "dialog", "aria-modal": true, tabIndex: -1 } : {})}
    >
      {sheet && (
        <button
          type="button"
          className="ts-notepanel__grab"
          aria-label={t("notePanel.close")}
          onClick={onClose}
        />
      )}
      <div className="ts-notepanel__head">
        {modes.length > 1 ? (
          <div ref={modesRef} className="ts-notepanel__modes" role="group" aria-label={t("notePanel.modes")}>
            {modes.map((mode) => (
              <button
                key={mode.id}
                type="button"
                className={
                  mode.id === spec.id
                    ? "ts-notepanel__mode ts-eyebrow ts-notepanel__mode--on"
                    : "ts-notepanel__mode ts-eyebrow"
                }
                aria-pressed={mode.id === spec.id}
                onClick={() => onSelect(mode.id)}
              >
                {t(mode.title)}
              </button>
            ))}
          </div>
        ) : (
          <span className="ts-eyebrow ts-notepanel__mode--on">{t(spec.title)}</span>
        )}
        <span className="ts-spacer" />
        {spec.badge !== null && <span className="ts-notepanel__tag">{spec.badge}</span>}
      </div>
      <div className="ts-notepanel__body">{spec.render()}</div>
    </aside>
  );

  if (!sheet) return panel;
  return (
    <div
      className="ts-notepanel__overlay"
      onClick={(event) => {
        // Тап мимо шторки, а не по ней самой.
        if (event.target === event.currentTarget) onClose();
      }}
    >
      {panel}
    </div>
  );
}
