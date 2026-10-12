import { useEffect } from "react";
import { useI18n } from "../i18n/index.js";
import type { MoveNotice as MoveNoticeState } from "./useMoveEntry.js";

/** Сколько висит сообщение об итоге (макет, экран «После броска»). */
const NOTICE_TTL_MS = 8000;

export interface MoveNoticeProps {
  notice: MoveNoticeState | null;
  /** Идёт перенос — «Отменить» недоступно. */
  busy: boolean;
  onUndo: () => void;
  onDismiss: () => void;
}

/**
 * Итог переноса в дереве (фича 023, FR-024): «Перенесено в …» с действием
 * «Отменить» либо причина отказа. Отменяется только последний перенос и только
 * пока сообщение видно — истории операций над файлами нет.
 */
export function MoveNotice({
  notice,
  busy,
  onUndo,
  onDismiss,
}: MoveNoticeProps): React.JSX.Element | null {
  const { t } = useI18n();

  // Отказ не исчезает сам: его нужно успеть прочитать. Итог успешного переноса
  // и отмены — исчезает.
  const expires = notice !== null && notice.kind !== "failed";
  useEffect(() => {
    if (!expires) return undefined;
    const timer = window.setTimeout(onDismiss, NOTICE_TTL_MS);
    return () => window.clearTimeout(timer);
  }, [expires, notice, onDismiss]);

  if (notice === null) return null;

  if (notice.kind === "failed") {
    return (
      <div className="ts-tree-notice ts-tree-notice--failed" role="alert">
        <span className="ts-tree-notice__text">{notice.message(t)}</span>
        <button type="button" className="ts-btn ts-btn--quiet" onClick={onDismiss}>
          {t("common.close")}
        </button>
      </div>
    );
  }

  return (
    <div className="ts-tree-notice" role="status">
      <span className="ts-tree-notice__text">
        {notice.kind === "done"
          ? t("tree.move.done", { folder: notice.folder })
          : t("tree.move.undone")}
      </span>
      {notice.kind === "done" && (
        <button type="button" className="ts-btn ts-btn--outline" disabled={busy} onClick={onUndo}>
          {t("tree.move.undo")}
        </button>
      )}
    </div>
  );
}
