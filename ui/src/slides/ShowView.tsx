"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { SlideLabel } from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { SlideImage, type SlidePage } from "./SlideImage.js";
import { useFullscreen } from "./useFullscreen.js";

/** Панель прячется через это время после последнего движения (A2). */
export const HUD_HIDE_MS = 2000;
/** Полоса «Выйти» на телефоне держится после касания середины (A8). */
export const TOUCH_BAR_MS = 3000;
/** Подсказка о полноэкранном режиме гаснет сама (A6). */
export const FULLSCREEN_HINT_MS = 6000;
/** Сдвиг пальца, с которого касание считается свайпом, а не нажатием. */
const SWIPE_MIN_PX = 48;

/**
 * Пропсы окна показа. Заметок докладчика здесь нет намеренно: показ идёт на
 * проектор, а `notes` принадлежат только виду докладчика (FR-006) — поле не
 * передаётся в этот компонент, поэтому и нарисовать его он не может.
 */
export interface ShowViewProps {
  /** Текущая страница; `null` — показывать нечего (A5). */
  page: SlidePage | null;
  label: SlideLabel | null;
  /** Сборка текущего текста не удалась, показана прошлая (A4). */
  stale: boolean;
  /** Строка первой ошибки (с единицы) для плашки A4. */
  errorLine: number | null;
  /** Причина пустого экрана — текст первой ошибки (A5). */
  emptyReason: string | null;
  /** Набранные цифры номера слайда (A3). */
  gotoBuffer: string | null;
  /** Есть ли в оболочке вид докладчика (FR-009). */
  canPresent: boolean;
  /**
   * Телефон (A8): вместо панели — зоны касания 30/40/30 %, свайп и полоса
   * «Выйти» со счётчиком на 3 с по касанию середины.
   */
  touch?: boolean;
  onPrevious: () => void;
  onNext: () => void;
  onPresenter: () => void;
  onExit: () => void;
  onRevealError: () => void;
  onClose: () => void;
}

/** Окно показа (A2–A6, A8): слайд, панель, поле номера, плашки. */
export function ShowView({
  page,
  label,
  stale,
  errorLine,
  emptyReason,
  gotoBuffer,
  canPresent,
  touch = false,
  onPrevious,
  onNext,
  onPresenter,
  onExit,
  onRevealError,
  onClose,
}: ShowViewProps): React.JSX.Element {
  const { t, formatNumber } = useI18n();
  const { fullscreen, request } = useFullscreen();

  // Панель: видна при движении мыши и касании, через 2 с прячется. Пока в ней
  // фокус (клавиатура), она остаётся видимой — иначе фокус «пропал бы» (CSS).
  const [hudVisible, setHudVisible] = useState(true);
  const hudTimer = useRef<number | null>(null);
  const wake = useCallback(() => {
    setHudVisible(true);
    if (hudTimer.current !== null) window.clearTimeout(hudTimer.current);
    hudTimer.current = window.setTimeout(() => setHudVisible(false), HUD_HIDE_MS);
  }, []);
  useEffect(() => {
    wake();
    return () => {
      if (hudTimer.current !== null) window.clearTimeout(hudTimer.current);
    };
  }, [wake]);

  // Подсказка A6: окно не развёрнуто. Первый жест пробует развернуть окно сам
  // (`requestFullscreen` разрешён только по жесту) и гасит подсказку.
  const [hint, setHint] = useState(!touch && !fullscreen);
  useEffect(() => {
    if (!hint) return undefined;
    const timer = window.setTimeout(() => setHint(false), FULLSCREEN_HINT_MS);
    return () => window.clearTimeout(timer);
  }, [hint]);
  useEffect(() => {
    if (touch) return undefined;
    const onGesture = (): void => {
      setHint(false);
      void request();
    };
    window.addEventListener("keydown", onGesture, { once: true });
    window.addEventListener("pointerdown", onGesture, { once: true });
    return () => {
      window.removeEventListener("keydown", onGesture);
      window.removeEventListener("pointerdown", onGesture);
    };
  }, [touch, request]);

  // Телефон: полоса «Выйти» по касанию середины, свайп листает.
  const [bar, setBar] = useState(false);
  const barTimer = useRef<number | null>(null);
  const showBar = useCallback(() => {
    setBar(true);
    if (barTimer.current !== null) window.clearTimeout(barTimer.current);
    barTimer.current = window.setTimeout(() => setBar(false), TOUCH_BAR_MS);
  }, []);
  useEffect(
    () => () => {
      if (barTimer.current !== null) window.clearTimeout(barTimer.current);
    },
    [],
  );
  const swipe = useRef<{ x: number; y: number; swiped: boolean } | null>(null);
  const onPointerDown = (event: React.PointerEvent): void => {
    if (touch) swipe.current = { x: event.clientX, y: event.clientY, swiped: false };
    else wake();
  };
  const onPointerUp = (event: React.PointerEvent): void => {
    const start = swipe.current;
    if (!touch || start === null) return;
    const dx = event.clientX - start.x;
    const dy = event.clientY - start.y;
    if (Math.abs(dx) >= SWIPE_MIN_PX && Math.abs(dx) > Math.abs(dy)) {
      start.swiped = true;
      if (dx < 0) onNext();
      else onPrevious();
    }
  };
  const zoneClick = (action: () => void) => (): void => {
    // Свайп уже отлистал — «нажатие», которым он закончился, не повторяет его.
    if (swipe.current?.swiped === true) {
      swipe.current = null;
      return;
    }
    action();
  };

  if (page === null || label === null) {
    return (
      <div className="ts-show ts-show--empty">
        <div className="ts-show__empty">
          <h1 className="ts-show__empty-title">{t("slides.empty.title")}</h1>
          <p className="ts-show__empty-body">{t("slides.empty.body")}</p>
          {emptyReason !== null && <pre className="ts-show__diag">{emptyReason}</pre>}
          <div className="ts-show__empty-actions">
            <button type="button" className="ts-btn ts-btn--accent" onClick={onExit} autoFocus>
              {t("slides.empty.back")}
            </button>
            <button type="button" className="ts-btn ts-btn--outline" onClick={onClose}>
              {t("slides.empty.close")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  const counter = (
    <span className="ts-show__counter" role="status" aria-live="polite">
      <span>{formatNumber(label.current)}</span>
      <i>/ {formatNumber(label.total)}</i>
      {!touch && label.step !== undefined && label.steps !== undefined && (
        <i className="ts-show__step">
          · {t("slides.step", { step: label.step, steps: label.steps })}
        </i>
      )}
    </span>
  );

  return (
    <div
      className={touch ? "ts-show ts-show--touch" : "ts-show"}
      onPointerMove={touch ? undefined : wake}
      onPointerDown={onPointerDown}
      onPointerUp={onPointerUp}
    >
      <SlideImage
        className="ts-show__stage"
        page={page}
        label={t("slides.slideAlt", { current: label.current, total: label.total })}
      />

      {stale && (
        <div className="ts-show__banner" role="status">
          <span className="ts-show__banner-dot" aria-hidden="true" />
          <span className="ts-show__banner-text">
            {errorLine === null ? t("slides.stale") : t("slides.staleAt", { line: errorLine })}
          </span>
          <button type="button" className="ts-show__banner-action" onClick={onRevealError}>
            {t("slides.toError")}
          </button>
        </div>
      )}

      {hint && !stale && gotoBuffer === null && (
        <div className="ts-show__banner ts-show__banner--info" role="status">
          <span className="ts-show__banner-dot" aria-hidden="true" />
          <span className="ts-show__banner-text">{t("slides.fullscreenHint")}</span>
        </div>
      )}

      {gotoBuffer !== null && (
        <div className="ts-show__goto" role="status" aria-label={t("slides.goto.label")}>
          <div className="ts-show__goto-box">
            <span className="ts-show__goto-num">{gotoBuffer}</span>
            <small>{t("slides.goto.hint", { total: label.total })}</small>
          </div>
        </div>
      )}

      {touch ? (
        <>
          <button
            type="button"
            className="ts-show__zone ts-show__zone--prev"
            aria-label={t("slides.previous")}
            onClick={zoneClick(onPrevious)}
          />
          <button
            type="button"
            className="ts-show__zone ts-show__zone--menu"
            aria-label={t("slides.toolbar")}
            onClick={zoneClick(showBar)}
          />
          <button
            type="button"
            className="ts-show__zone ts-show__zone--next"
            aria-label={t("slides.next")}
            onClick={zoneClick(onNext)}
          />
          {bar && (
            <div className="ts-show__topbar" role="toolbar" aria-label={t("slides.toolbar")}>
              <button type="button" className="ts-show__btn" onClick={onExit}>
                <Icon name="x" size={14} />
                {t("slides.overlay.exit")}
              </button>
              {counter}
            </div>
          )}
        </>
      ) : (
        <div
          className={hudVisible ? "ts-show__hud" : "ts-show__hud ts-show__hud--hidden"}
          role="toolbar"
          aria-label={t("slides.toolbar")}
        >
          <button
            type="button"
            className="ts-show__btn"
            aria-label={t("slides.previous")}
            title={t("slides.previous")}
            onClick={onPrevious}
          >
            <Icon name="chevron" size={14} className="ts-icon--flip" />
          </button>
          {counter}
          <button
            type="button"
            className="ts-show__btn"
            aria-label={t("slides.next")}
            title={t("slides.next")}
            onClick={onNext}
          >
            <Icon name="chevron" size={14} />
          </button>
          <span className="ts-show__sep" aria-hidden="true" />
          {canPresent && (
            <button
              type="button"
              className="ts-show__btn"
              title={t("slides.presenterHint")}
              onClick={onPresenter}
            >
              <Icon name="presenter" size={14} />
              <span className="ts-show__btn-label">{t("slides.presenter")}</span>
            </button>
          )}
          {!fullscreen && (
            <button
              type="button"
              className="ts-show__btn"
              aria-label={t("slides.fullscreen")}
              title={t("slides.fullscreen")}
              onClick={() => {
                setHint(false);
                void request();
              }}
            >
              <Icon name="fullscreen" size={14} />
            </button>
          )}
          <button
            type="button"
            className="ts-show__btn"
            aria-label={t("slides.exit")}
            title={t("slides.exit")}
            onClick={onExit}
          >
            <Icon name="x" size={14} />
            <span className="ts-show__btn-label">{t("slides.exitKey")}</span>
          </button>
        </div>
      )}
    </div>
  );
}
