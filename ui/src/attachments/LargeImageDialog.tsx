"use client";

import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import type { LargeImageRequest } from "./useAttachments.js";

export interface LargeImageDialogProps {
  request: LargeImageRequest;
  onAnswer: (accept: boolean) => void;
}

/**
 * Подтверждение изображения больше 20 МБ (фича 016, FR-013, макет 1b).
 * Уменьшать картинку приложение не берётся — только спрашивает. `Escape` —
 * «Отмена»: файл ещё не записан, и отказ ничего не теряет.
 */
export function LargeImageDialog({ request, onAnswer }: LargeImageDialogProps): React.JSX.Element {
  const i18n = useI18n();
  const { t } = i18n;
  useDismissOnEscape(() => onAnswer(false));
  return (
    <div
      className="ts-dialog ts-attach-large"
      role="alertdialog"
      aria-modal="true"
      aria-label={t("attachments.large.title")}
    >
      <strong>{t("attachments.large.title")}</strong>
      <p className="ts-attach-large__file">
        <span className="ts-attach-large__name">{request.name}</span> ·{" "}
        {i18n.formatBytes(request.size)}
      </p>
      <p>{t("attachments.large.asIs")}</p>
      <p className="ts-attach-large__path">
        {t("attachments.large.target", { folder: request.folder })}
      </p>
      <div role="group" className="ts-attach-large__actions">
        <button type="button" className="ts-btn" onClick={() => onAnswer(false)} autoFocus>
          {t("attachments.large.cancel")}
        </button>
        <button type="button" className="ts-btn ts-btn--accent" onClick={() => onAnswer(true)}>
          {t("attachments.large.insert")}
        </button>
      </div>
    </div>
  );
}
