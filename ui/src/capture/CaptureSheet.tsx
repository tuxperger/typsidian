"use client";

import { useEffect, useId, useRef } from "react";
import type { Capture } from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import { useI18n, type Translate } from "../i18n/index.js";
import { message } from "../i18n/messages.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import type { CaptureSheetModel } from "./useCapture.js";

/**
 * Лист захвата (фича 022, макет D1–D3): что захватывается, куда, приписка и
 * «Сохранить». Только показывает модель `useCapture` и отдаёт ей действия.
 *
 * Лист поверх рабочей области: на узком окне прижат к низу и во всю ширину, на
 * широком — той же шириной по центру внизу.
 */
export interface CaptureSheetProps {
  model: CaptureSheetModel;
  /** Имя vault для заголовка; `null` — vault не открыт (D3). */
  vaultName: string | null;
}

export function CaptureSheet({ model, vaultName }: CaptureSheetProps): React.JSX.Element {
  const { t, formatBytes } = useI18n();
  const titleId = useId();
  const sheetRef = useRef<HTMLElement>(null);
  const primaryRef = useRef<HTMLButtonElement>(null);
  const textRef = useRef<HTMLTextAreaElement>(null);
  const { failure, capture } = model;
  const noVault = failure === "noVault";

  useDismissOnEscape(model.close);

  // Фокус внутри листа с первого кадра: для своей мысли — в поле текста, для
  // чужого содержимого — на «Сохранить» (SC-001: три касания). Прежний фокус
  // возвращается при закрытии.
  const captureId = capture.id;
  useEffect(() => {
    const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    (model.manual ? textRef.current : primaryRef.current)?.focus();
    return () => before?.focus();
    // Только появление листа и смена захвата.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [captureId]);

  // Табуляция не уходит из листа под затемнение.
  const trapTab = (event: React.KeyboardEvent<HTMLElement>): void => {
    if (event.key !== "Tab") return;
    const focusable = [
      ...(sheetRef.current?.querySelectorAll<HTMLElement>(
        "button:not(:disabled), textarea, input, select",
      ) ?? []),
    ];
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

  const canSave = !model.busy && !(model.manual && model.text.trim() === "");
  const time = timeLabel(capture);
  const targetKind = model.target.kind;
  const recentValue = model.target.kind === "Note" ? model.target.path : "";

  return (
    <>
      <div className="ts-capture__scrim" aria-hidden="true" onClick={model.close} />
      <section
        ref={sheetRef}
        className="ts-capture"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-label={t("capture.sheet.label")}
        onKeyDown={trapTab}
      >
        <div className="ts-capture__grab" aria-hidden="true" />

        <header className="ts-capture__head">
          <h2 id={titleId} className="ts-capture__title">
            {t("capture.sheet.title")}
          </h2>
          {vaultName !== null && <span className="ts-capture__vault">{vaultName}</span>}
          <button
            type="button"
            className="ts-btn ts-btn--icon"
            title={t("capture.sheet.close")}
            aria-label={t("capture.sheet.close")}
            onClick={model.close}
          >
            <Icon name="x" size={14} />
          </button>
        </header>

        {model.manual ? (
          <textarea
            ref={textRef}
            className="ts-capture__text"
            value={model.text}
            rows={4}
            aria-label={t("capture.manual.label")}
            placeholder={t("capture.manual.placeholder")}
            spellCheck
            onChange={(event) => model.setText(event.target.value)}
          />
        ) : (
          <What capture={capture} model={model} t={t} formatBytes={formatBytes} />
        )}

        {failure !== null && (
          <div className="ts-capture__warn" role="alert">
            <strong>
              {noVault ? t("capture.failed.noVault.title") : t("capture.failed.title")}
            </strong>
            <span>
              {noVault
                ? t("capture.failed.noVault.body")
                : message(t, { code: "CaptureRefused", reason: failure })}
            </span>
          </div>
        )}

        {!noVault && (
          <>
            <div className="ts-capture__dest" role="group" aria-label={t("capture.dest.label")}>
              <button
                type="button"
                className="ts-capture__seg"
                aria-pressed={targetKind === "Inbox"}
                onClick={() => model.setTarget({ kind: "Inbox" })}
              >
                {t("capture.dest.inbox")}
              </button>
              <button
                type="button"
                className="ts-capture__seg"
                aria-pressed={targetKind === "Daily"}
                onClick={() => model.setTarget({ kind: "Daily" })}
              >
                {t("capture.dest.daily")}
              </button>
              <select
                className="ts-capture__seg ts-capture__seg--select"
                aria-label={t("capture.dest.recent")}
                data-active={targetKind === "Note"}
                value={recentValue}
                onChange={(event) => {
                  const path = event.target.value;
                  if (path !== "") model.setTarget({ kind: "Note", path });
                }}
              >
                <option value="">
                  {model.recent.length === 0
                    ? t("capture.dest.recentNone")
                    : t("capture.dest.recentChoose")}
                </option>
                {model.recent.map((path) => (
                  <option key={path} value={path}>
                    {path}
                  </option>
                ))}
              </select>
            </div>

            <input
              type="text"
              className="ts-capture__note"
              value={model.note}
              aria-label={t("capture.note.label")}
              placeholder={t("capture.note.placeholder")}
              onChange={(event) => model.setNote(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && canSave) {
                  event.preventDefault();
                  model.save();
                }
              }}
            />
          </>
        )}

        <div className="ts-capture__actions">
          {noVault ? (
            <button
              ref={primaryRef}
              type="button"
              className="ts-btn ts-btn--accent ts-capture__primary"
              onClick={model.close}
            >
              {t("capture.failed.open")}
            </button>
          ) : (
            <button
              ref={primaryRef}
              type="button"
              className="ts-btn ts-btn--accent ts-capture__primary"
              disabled={!canSave}
              onClick={model.save}
            >
              {model.busy
                ? t("capture.saving")
                : failure !== null
                  ? t("capture.failed.retry")
                  : t("capture.save")}
            </button>
          )}
          {failure !== null && (
            <button type="button" className="ts-btn ts-btn--outline" onClick={model.discard}>
              {t("capture.failed.discard")}
            </button>
          )}
        </div>

        <p className="ts-capture__hint">
          {failure !== null
            ? t("capture.failed.pending", { count: model.waiting })
            : hint(t, model, time)}
        </p>
        {failure === null && model.more > 0 && (
          <p className="ts-capture__hint">{t("capture.queue.more", { count: model.more })}</p>
        )}
      </section>
    </>
  );
}

/** Что именно захватывается (D1, D2). */
function What({
  capture,
  model,
  t,
  formatBytes,
}: {
  capture: Capture;
  model: CaptureSheetModel;
  t: Translate;
  formatBytes: (bytes: number) => string;
}): React.JSX.Element {
  const content = capture.content;
  if (content.kind === "Image") {
    return (
      <figure className="ts-capture__thumb">
        {model.image !== null && (
          <img src={model.image.url} alt={t("capture.image.alt", { name: content.name })} />
        )}
        <figcaption>
          {model.image === null
            ? content.name
            : t("capture.image.caption", {
                name: content.name,
                size: formatBytes(model.image.bytes),
              })}
        </figcaption>
      </figure>
    );
  }
  if (content.kind === "Link") {
    return (
      <div className="ts-capture__what">
        <b>{content.title ?? content.url}</b>
        {content.title !== undefined && <small>{content.url}</small>}
      </div>
    );
  }
  return (
    <div className="ts-capture__what">
      <b className="ts-capture__what-text">{content.text}</b>
    </div>
  );
}

function timeLabel(capture: Capture): string {
  const pad = (value: number): string => String(value).padStart(2, "0");
  return `${pad(capture.at.hour)}:${pad(capture.at.minute)}`;
}

function hint(t: Translate, model: CaptureSheetModel, time: string): string {
  if (model.destination === null) return t("capture.hint.unknown");
  return model.capture.content.kind === "Image"
    ? t("capture.hint.image", { path: model.destination })
    : t("capture.hint.append", { path: model.destination, time });
}
