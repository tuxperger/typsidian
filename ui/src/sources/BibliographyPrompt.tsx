"use client";

import { useI18n } from "../i18n/index.js";
import type { BibliographyOffer } from "../wasm-core/typsidian_core.js";

export interface BibliographyPromptProps {
  offer: Exclude<BibliographyOffer, { kind: "None" }>;
  onAccept: () => void;
  onLater: () => void;
}

/**
 * Ненавязчивый запрос у нижнего края редактора (фича 017, FR-004): без
 * нажатия «Добавить» текст заметки не меняется.
 */
export function BibliographyPrompt({
  offer,
  onAccept,
  onLater,
}: BibliographyPromptProps): React.JSX.Element {
  const { t } = useI18n();
  return (
    <div className="ts-cite-offer" role="status">
      <span>
        {offer.kind === "Add" ? t("sources.offer.add") : t("sources.offer.extend", { file: offer.file })}
      </span>
      <span className="ts-spacer" />
      <button type="button" className="ts-btn ts-btn--accent" onClick={onAccept}>
        {t("sources.offer.accept")}
      </button>
      <button type="button" className="ts-btn ts-btn--quiet" onClick={onLater}>
        {t("sources.offer.later")}
      </button>
    </div>
  );
}
