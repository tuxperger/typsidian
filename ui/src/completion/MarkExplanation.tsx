"use client";

import { useState } from "react";
import { useI18n } from "../i18n/index.js";
import type { ReferenceMark } from "../wasm-core/typsidian_core.js";
import { markText } from "./describe.js";

export interface MarkExplanationProps {
  /** Отметка, на которой стоит каретка. */
  mark: ReferenceMark;
  readOnly: boolean;
  /** Создать заметку по пути ссылки; `false` — отказ. */
  onCreateNote: (path: string) => Promise<boolean>;
}

/**
 * Пояснение отметки у каретки (фича 018, макет 1c): причина, описание и для
 * «заметки нет» — «Создать заметку». По наведению то же пояснение даёт
 * подсказка общего механизма отметок; на телефоне наведения нет, поэтому
 * здесь — по каретке.
 */
export function MarkExplanation({
  mark,
  readOnly,
  onCreateNote,
}: MarkExplanationProps): React.JSX.Element {
  const { t } = useI18n();
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const { title, detail } = markText(t, mark.kind);
  const kind = mark.kind;
  return (
    <div className="ts-mark-explain" role="status">
      <span className="ts-mark-explain__title">{title}</span>
      <span className="ts-mark-explain__detail">{failed ? t("completion.mark.createFailed") : detail}</span>
      <span className="ts-spacer" />
      {kind.kind === "MissingNote" && !readOnly && (
        <button
          type="button"
          className="ts-btn ts-btn--accent"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            setFailed(false);
            void onCreateNote(kind.create_path).then((created) => {
              setBusy(false);
              setFailed(!created);
            });
          }}
        >
          {t("completion.mark.createNote")}
        </button>
      )}
    </div>
  );
}
