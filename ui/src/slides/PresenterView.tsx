"use client";

import { useEffect, useState } from "react";
import type {
  Diagnostic,
  PageSlideInfo,
  ShowAction,
  ShowState,
  SlideLabel,
} from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { SlideImage, type SlidePage } from "./SlideImage.js";

export interface PresenterViewProps {
  state: ShowState;
  label: SlideLabel;
  pages: SlidePage[];
  /** Сведения о страницах: заметки докладчика (только здесь, FR-006). */
  slides: PageSlideInfo[];
  stale: boolean;
  /** Первая ошибка — для пометки «показана последняя удачная сборка». */
  failure: Diagnostic | undefined;
  onAction: (action: ShowAction) => void;
  onExit: () => void;
  onRevealError: () => void;
}

/** «чч:мм:сс» от длительности в миллисекундах. */
export function formatElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const part = (value: number): string => String(value).padStart(2, "0");
  return `${part(Math.floor(total / 3600))}:${part(Math.floor(total / 60) % 60)}:${part(total % 60)}`;
}

/** Часы и время с начала; обновляются раз в секунду. */
function useNow(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  return now;
}

/** Вид докладчика (A7): текущий и следующий слайд, заметки, часы, лента. */
export function PresenterView({
  state,
  label,
  pages,
  slides,
  stale,
  failure,
  onAction,
  onExit,
  onRevealError,
}: PresenterViewProps): React.JSX.Element {
  const { t, lang, formatNumber } = useI18n();
  const now = useNow();

  const current = pages[state.page];
  const nextIndex = label.next_page;
  const next = nextIndex === undefined ? undefined : pages[nextIndex];
  const notes = slides[state.page]?.notes;
  const paragraphs = notes === undefined ? [] : notes.split(/\n\s*\n/).filter((text) => text.trim() !== "");
  const errorLine = failure?.location?.line;

  return (
    <div className="ts-presenter" role="region" aria-label={t("slides.presenter.label")}>
      <div className="ts-presenter__top">
        <div className="ts-presenter__clock">
          <small>{t("slides.presenter.clock")}</small>
          <time dateTime={new Date(now).toISOString()}>
            {new Intl.DateTimeFormat(lang, { hour: "2-digit", minute: "2-digit" }).format(now)}
          </time>
        </div>
        <div className="ts-presenter__clock ts-presenter__clock--elapsed">
          <small>{t("slides.presenter.elapsed")}</small>
          <span>{formatElapsed(now - state.started_at_ms)}</span>
        </div>
        <span className="ts-spacer" />
        {stale && (
          <span className="ts-presenter__stale" role="status">
            <span className="ts-show__banner-dot" aria-hidden="true" />
            {errorLine === undefined ? t("slides.stale") : t("slides.staleAt", { line: errorLine })}
            <button type="button" className="ts-show__banner-action" onClick={onRevealError}>
              {t("slides.toError")}
            </button>
          </span>
        )}
        <span className="ts-show__counter ts-presenter__counter" role="status" aria-live="polite">
          <span>{formatNumber(label.current)}</span>
          <i>/ {formatNumber(label.total)}</i>
        </span>
        <button type="button" className="ts-btn ts-btn--outline" onClick={onExit}>
          {t("slides.presenter.end")}
        </button>
      </div>

      <div className="ts-presenter__current">
        <div className="ts-presenter__lbl">
          <span>{t("slides.presenter.onScreen")}</span>
          {label.step !== undefined && label.steps !== undefined && (
            <span>{t("slides.step", { step: label.step, steps: label.steps })}</span>
          )}
        </div>
        <div className="ts-presenter__slide ts-presenter__slide--current">
          {current !== undefined && (
            <SlideImage
              page={current}
              label={t("slides.slideAlt", { current: label.current, total: label.total })}
            />
          )}
        </div>
      </div>

      <div className="ts-presenter__side">
        <div className="ts-presenter__lbl">
          <span>{t("slides.presenter.upNext")}</span>
          {nextIndex !== undefined && <span>{formatNumber(nextIndex + 1)}</span>}
        </div>
        <div className="ts-presenter__slide ts-presenter__slide--next">
          {next !== undefined && nextIndex !== undefined && (
            <SlideImage
              page={next}
              label={t("slides.presenter.goToPage", { page: nextIndex + 1 })}
            />
          )}
        </div>
        <div className="ts-presenter__notes" role="region" aria-label={t("slides.presenter.notes")} tabIndex={0}>
          {paragraphs.map((text, index) => (
            <p key={index}>{text}</p>
          ))}
        </div>
      </div>

      <div className="ts-presenter__bottom">
        <button
          type="button"
          className="ts-btn ts-btn--outline ts-btn--icon"
          aria-label={t("slides.previous")}
          onClick={() => onAction({ kind: "previous" })}
        >
          <Icon name="chevron" size={14} className="ts-icon--flip" />
        </button>
        <ol className="ts-presenter__strip" aria-label={t("slides.presenter.strip")}>
          {pages.map((page, index) => (
            <li key={`${index}:${page.digest}`}>
              <button
                type="button"
                className={
                  index === state.page
                    ? "ts-presenter__thumb ts-presenter__thumb--current"
                    : index < state.page
                      ? "ts-presenter__thumb ts-presenter__thumb--done"
                      : "ts-presenter__thumb"
                }
                aria-label={t("slides.presenter.goToPage", { page: index + 1 })}
                aria-current={index === state.page ? "true" : undefined}
                onClick={() => onAction({ kind: "goToPage", index })}
              />
            </li>
          ))}
        </ol>
        <button
          type="button"
          className="ts-btn ts-btn--outline ts-btn--icon"
          aria-label={t("slides.next")}
          onClick={() => onAction({ kind: "next" })}
        >
          <Icon name="chevron" size={14} />
        </button>
      </div>
    </div>
  );
}
