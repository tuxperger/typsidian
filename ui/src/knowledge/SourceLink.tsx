"use client";

import type { SourceRef } from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { isLessonPath, sourceName, type OpenSource } from "./knowledgeText.js";

// Источник утверждения (макет A): урок со ссылкой «открыть на месте» либо заметка; удалённый
// урок с оставленным вкладом — зачёркнутым именем и пометкой, ссылки у него нет.

export function SourceLink({
  source,
  onOpen,
  compact = false,
}: {
  source: SourceRef;
  onOpen: OpenSource;
  /** Без значка и без места — ячейка таблицы. */
  compact?: boolean;
}): React.JSX.Element {
  const { t } = useI18n();
  const name = sourceName(source.path);
  const lesson = isLessonPath(source.path);
  const place = source.event !== undefined ? t("knowledge.source.event", { n: source.event + 1 }) : null;
  return (
    <span className="ts-kg__src">
      {!compact && (
        <span className="ts-kg__src-icon" aria-hidden="true">
          {lesson ? "◆" : <Icon name="file" size={12} />}
        </span>
      )}
      {source.removed ? (
        <>
          <span className="ts-kg__gone">{name}</span>
          <span className="ts-kg__hint">{t("knowledge.source.removed")}</span>
        </>
      ) : (
        <>
          <button
            type="button"
            className="ts-lesson__link ts-kg__link"
            aria-label={t("knowledge.source.open", { name })}
            onClick={() => onOpen(source.path, source.event)}
          >
            {name}
          </button>
          {place !== null && <span className="ts-kg__hint">{place}</span>}
        </>
      )}
    </span>
  );
}
