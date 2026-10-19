"use client";

import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import type { NotePanelModeSpec, PanelModeId } from "./modes.js";

// Боковая шторка колонки заметки для узкого окна (фича 015, макет
// «Typsidian Оглавление» 1b, research.md §11): та же колонка с теми же
// режимами, только панелью 280px справа поверх редактора на скриме. Между
// встроенной колонкой (≥1100px) и шторкой снизу (≤720px, фича 013).

/** Ширина `.ts-body`, ниже которой колонка уходит в боковую шторку. */
export const NOTE_PANEL_DRAWER_BELOW_PX = 1100;

export interface NotePanelDrawerProps {
  modes: readonly NotePanelModeSpec[];
  active: PanelModeId;
  onSelect: (mode: PanelModeId) => void;
  /** Закрыть: скрим, метка `esc`, переход к пункту. Фокус возвращает вызывающий. */
  onClose: () => void;
  /**
   * Escape. Отдельно от `onClose`: у режима «Структура» он сначала очищает
   * непустой фильтр и только потом закрывает шторку.
   */
  onEscape: () => void;
}

export function NotePanelDrawer({
  modes,
  active,
  onSelect,
  onClose,
  onEscape,
}: NotePanelDrawerProps): React.JSX.Element {
  const { t } = useI18n();
  const spec = modes.find((mode) => mode.id === active) ?? modes[0];
  const panelRef = useRef<HTMLElement | null>(null);

  useDismissOnEscape(onEscape);

  // Фокус уходит в шторку при открытии; режим, которому нужен свой фокус
  // (поле фильтра оглавления), ставит его сам — тогда панель не перебивает.
  useEffect(() => {
    const panel = panelRef.current;
    if (panel !== null && !panel.contains(document.activeElement)) panel.focus();
  }, []);

  // Фокус заперт внутри шторки, пока она открыта (макет 1b, модальность).
  const onKeyDown = (event: React.KeyboardEvent<HTMLElement>): void => {
    if (event.key !== "Tab") return;
    const panel = panelRef.current;
    if (panel === null) return;
    const focusable = Array.from(
      panel.querySelectorAll<HTMLElement>(
        'button:not(:disabled), input:not(:disabled), [tabindex="0"], a[href]',
      ),
    );
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (first === undefined || last === undefined) return;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  if (spec === undefined) return <></>;

  return (
    <div
      className="ts-notepanel-drawer"
      onClick={(event) => {
        // Щелчок мимо панели, а не по ней.
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <aside
        ref={panelRef}
        className="ts-notepanel ts-notepanel-drawer__panel"
        role="dialog"
        aria-modal="true"
        aria-label={t("notePanel.label")}
        tabIndex={-1}
        onKeyDown={onKeyDown}
      >
        <div className="ts-notepanel__head">
          {modes.length > 1 ? (
            <div className="ts-notepanel__modes" role="group" aria-label={t("notePanel.modes")}>
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
          <button
            type="button"
            className="ts-notepanel__tag ts-notepanel-drawer__esc"
            aria-label={t("outline.drawer.close")}
            onClick={onClose}
          >
            {t("outline.drawer.esc")}
          </button>
        </div>
        <div className="ts-notepanel__body">{spec.render()}</div>
        <div className="ts-notepanel-drawer__foot" aria-hidden="true">
          {t("outline.drawer.footer")}
        </div>
      </aside>
    </div>
  );
}

/**
 * Уже ли `.ts-body` порога боковой шторки. Меряется сам элемент, а не окно:
 * ширину колонке оставляют боковая панель и предпросмотр. Первый кадр —
 * `false`, как у `useNarrowViewport` (серверная разметка Next.js).
 */
export function useBodyBelow(
  body: React.RefObject<HTMLElement | null>,
  px: number = NOTE_PANEL_DRAWER_BELOW_PX,
): boolean {
  const [below, setBelow] = useState(false);
  // `.ts-body` появляется позже хука (сначала экран выбора vault) и
  // пересоздаётся при уходе в граф: элемент сверяется на каждой отрисовке, и
  // наблюдатель переезжает на новый.
  const observed = useRef<HTMLElement | null>(null);
  const observer = useRef<ResizeObserver | null>(null);
  useEffect(() => {
    const node = body.current;
    if (node === observed.current) return;
    observer.current?.disconnect();
    observer.current = null;
    observed.current = node;
    if (node === null || typeof ResizeObserver === "undefined") return;
    const next = new ResizeObserver(([entry]) => {
      if (entry !== undefined) setBelow(entry.contentRect.width < px);
    });
    next.observe(node);
    observer.current = next;
  });
  useEffect(() => () => observer.current?.disconnect(), []);
  return below;
}
