"use client";

import { useEffect, useId, useState } from "react";
import { useI18n } from "../i18n/index.js";

// Поле «Исследовать…» под лентой (фича 056, макет A): вопрос, «В вебе» и «В PDF…».
// «В вебе» недоступна, пока поиск выключен или подключение его не умеет; почему — подписью.

export type WebState = "ok" | "off" | "unavailable";

export interface ResearchInputProps {
  web: WebState;
  onWeb: (question: string) => void;
  onPdf: (question: string) => void;
  onOpenSettings: () => void;
  /** Отказ запуска (`SearchDisabled`, `RateLimited`…): текст уже локализован. */
  error: string | null;
  /** Поле тронуто (фокус, указатель): вкладка обновляет настройки поиска. */
  onTouch: () => void;
  /** Меняется, когда запрос ушёл не из поля (PDF): текст в поле больше не нужен. */
  clearToken: number;
}

export function ResearchInput({
  web,
  onWeb,
  onPdf,
  onOpenSettings,
  error,
  onTouch,
  clearToken,
}: ResearchInputProps): React.JSX.Element {
  const { t } = useI18n();
  const id = useId();
  const [text, setText] = useState("");
  useEffect(() => {
    if (clearToken > 0) setText("");
  }, [clearToken]);
  const question = text.trim();
  const webReady = web === "ok" && question !== "";
  const whyOff = web === "off" ? t("research.input.webOff") : web === "unavailable" ? t("research.input.webUnavailable") : null;

  return (
    <div className="ts-research__bar" onFocusCapture={onTouch} onPointerEnter={onTouch}>
      <div className="ts-research__row">
        <input
          className="ts-field__input ts-research__input"
          aria-label={t("research.input.label")}
          placeholder={t("research.input.placeholder")}
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && webReady) {
              onWeb(question);
              setText("");
            }
          }}
        />
        <button
          type="button"
          className="ts-btn"
          disabled={!webReady}
          title={whyOff ?? undefined}
          aria-describedby={whyOff === null ? undefined : `${id}-why`}
          onClick={() => {
            onWeb(question);
            setText("");
          }}
        >
          {t("research.input.web")}
        </button>
        <button
          type="button"
          className="ts-btn"
          onClick={() => onPdf(question)}
        >
          {t("research.input.pdf")}
        </button>
      </div>
      {whyOff !== null && (
        <p className="ts-lesson__hint" id={`${id}-why`}>
          {web === "off" ? t("research.box.off") : t("research.box.unavailable")}{" "}
          {web === "off" && (
            <button type="button" className="ts-lesson__link" onClick={onOpenSettings}>
              {t("research.box.offLink")}
            </button>
          )}
        </p>
      )}
      {error !== null && (
        <p className="ts-lesson__bad" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
