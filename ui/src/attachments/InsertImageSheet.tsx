"use client";

import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import type { PickSource } from "./pickFiles.js";

/** Какие источники даёт оболочка (research.md §2): web и рабочий стол — ни одного. */
export interface ImageSources {
  camera: boolean;
  gallery: boolean;
}

export interface InsertImageSheetProps {
  sources: ImageSources;
  onPick: (source: PickSource) => void;
  onClose: () => void;
}

/**
 * Шторка «Вставить изображение» на телефоне (фича 016, макет 1c): только
 * доступные действия — недоступное не показывается (FR-022).
 */
export function InsertImageSheet({
  sources,
  onPick,
  onClose,
}: InsertImageSheetProps): React.JSX.Element {
  const { t } = useI18n();
  useDismissOnEscape(onClose);
  const pick = (source: PickSource) => () => {
    onClose();
    onPick(source);
  };
  return (
    <div className="ts-attach-sheet" role="dialog" aria-modal="true" aria-label={t("attachments.sheet.title")}>
      <div className="ts-attach-sheet__head">
        <strong>{t("attachments.sheet.title")}</strong>
        <button type="button" className="ts-btn ts-btn--ghost" onClick={onClose}>
          {t("attachments.sheet.close")}
        </button>
      </div>
      {sources.camera && (
        <button type="button" className="ts-attach-sheet__action" onClick={pick("camera")}>
          {t("attachments.sheet.camera")}
        </button>
      )}
      {sources.gallery && (
        <button type="button" className="ts-attach-sheet__action" onClick={pick("gallery")}>
          {t("attachments.sheet.gallery")}
        </button>
      )}
      <button type="button" className="ts-attach-sheet__action" onClick={pick("files")}>
        {t("attachments.sheet.files")}
      </button>
    </div>
  );
}
