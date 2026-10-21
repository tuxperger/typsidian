"use client";

import { useI18n, type Translate } from "../i18n/index.js";
import type { AttachmentBatchView, IgnoreFile } from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import { itemFailure, itemName, problemText, refusalText } from "./describe.js";
import type { AttachmentNotice } from "./useAttachments.js";

export interface AttachmentNoticesProps {
  notice: AttachmentNotice;
  onCancel: () => void;
  onDismiss: () => void;
  /** Открыть редактор правил (012); `null` — синхронизации в оболочке нет. */
  onOpenRules: ((file: IgnoreFile) => void) | null;
}

/**
 * Панели вложений в ряду уведомлений (фича 016, макет 1b): прогресс, итог,
 * предупреждение об исключённой папке, отказ и сообщение о ссылке.
 */
export function AttachmentNotices({
  notice,
  onCancel,
  onDismiss,
  onOpenRules,
}: AttachmentNoticesProps): React.JSX.Element {
  const { t } = useI18n();
  const dismiss = (
    <button
      type="button"
      className="ts-btn ts-btn--ghost"
      aria-label={t("attachments.summary.dismiss")}
      onClick={onDismiss}
    >
      <Icon name="x" size={12} />
    </button>
  );

  switch (notice.kind) {
    case "progress": {
      const { view } = notice;
      return (
        <div className="ts-attach-panel ts-attach-panel--live" role="status">
          <div className="ts-attach-panel__body">
            <strong>
              {t("attachments.progress.title", { done: view.processed, total: view.total })}
            </strong>
            {notice.current !== null && (
              <span className="ts-attach-panel__path">
                {t("attachments.progress.current", {
                  name: notice.current,
                  folder: view.state.target_folder,
                })}
              </span>
            )}
            <span className="ts-hazard" aria-hidden="true">
              <span className="ts-hazard__fill" />
            </span>
            <span className="ts-attach-panel__note">{t("attachments.progress.note")}</span>
          </div>
          <button type="button" className="ts-btn" onClick={onCancel}>
            {t("attachments.progress.cancel")}
          </button>
        </div>
      );
    }
    case "link":
      return (
        <div className="ts-attach-panel ts-attach-panel--err" role="alert">
          <p>{t("attachments.link.dropped")}</p>
          {dismiss}
        </div>
      );
    case "refused": {
      const reason =
        notice.reason === "busy"
          ? t("attachments.refused.busy")
          : refusedReason(t, notice.view);
      return (
        <div className="ts-attach-panel ts-attach-panel--err" role="alert">
          <div className="ts-attach-panel__body">
            <strong>{t("attachments.refused.title")}</strong>
            {reason !== null && <span>{reason}</span>}
            {notice.reason !== "busy" && <span>{t("attachments.refused.unchanged")}</span>}
          </div>
          {dismiss}
        </div>
      );
    }
    case "summary":
      return (
        <>
          <Summary view={notice.view} notInserted={notice.notInserted} dismiss={dismiss} />
          {notice.view.state.excluded_by.length > 0 && notice.view.summary.stored > 0 && (
            <Excluded view={notice.view} onOpenRules={onOpenRules} />
          )}
        </>
      );
  }
}

function refusedReason(
  t: Translate,
  view: AttachmentBatchView | null,
): string | null {
  if (view === null) return null;
  const phase = view.state.phase;
  if (phase.kind === "Refused") return refusalText(t, phase.reason);
  const only = view.state.items[0];
  if (only === undefined) return null;
  const failure = itemFailure(t, only.status);
  return failure === null ? null : t("attachments.refused.reason", { name: itemName(t, only), reason: failure });
}

function Summary({
  view,
  notInserted,
  dismiss,
}: {
  view: AttachmentBatchView;
  notInserted: boolean;
  dismiss: React.JSX.Element;
}): React.JSX.Element {
  const { t } = useI18n();
  const { summary, state } = view;
  const inserted = notInserted ? 0 : summary.stored + summary.reused;
  const skipped = state.items.filter((item) => item.status.kind === "Skipped");
  // Ошибка — когда файл не записан или не вставлено ничего; пропуск части
  // файлов — обычный отчёт с красной строкой пропущенных (макет 1b).
  const problems = summary.failed > 0 || inserted === 0;
  return (
    <div
      className={problems ? "ts-attach-panel ts-attach-panel--err" : "ts-attach-panel"}
      role={problems ? "alert" : "status"}
    >
      <div className="ts-attach-panel__body">
        <strong>{t("attachments.summary.title", { inserted, total: view.total })}</strong>
        {notInserted && <span>{t("attachments.summary.notInserted")}</span>}
        <ul className="ts-attach-panel__list">
          {state.items.map((item, index) => {
            const status = item.status;
            if (status.kind === "Reused") {
              return (
                <li key={index}>
                  {itemName(t, item)} · {t("attachments.summary.reused")}
                </li>
              );
            }
            if (status.kind === "Stored" && status.saved_as !== undefined) {
              return (
                <li key={index}>
                  {item.incoming.name ?? t("attachments.unnamed")} ·{" "}
                  {t("attachments.summary.savedAs", { name: status.saved_as })}
                </li>
              );
            }
            // Вставленные — поимённо, как в макете 1b.
            if (status.kind === "Stored" && !notInserted) {
              return <li key={index}>{itemName(t, item)}</li>;
            }
            if (status.kind === "Failed") {
              return (
                <li key={index} className="ts-attach-panel__bad">
                  {t("attachments.refused.reason", {
                    name: itemName(t, item),
                    reason: problemText(t, status.problem),
                  })}
                </li>
              );
            }
            return null;
          })}
        </ul>
        {skipped.length > 0 && (
          <span className="ts-attach-panel__bad">
            {t("attachments.summary.skipped", {
              names: skipped.map((item) => itemName(t, item)).join(", "),
            })}
          </span>
        )}
        {summary.declined > 0 && (
          <span>{t("attachments.summary.declined", { count: summary.declined })}</span>
        )}
        {summary.cancelled > 0 && (
          <span>{t("attachments.summary.cancelled", { count: summary.cancelled })}</span>
        )}
      </div>
      {dismiss}
    </div>
  );
}

function Excluded({
  view,
  onOpenRules,
}: {
  view: AttachmentBatchView;
  onOpenRules: ((file: IgnoreFile) => void) | null;
}): React.JSX.Element {
  const { t } = useI18n();
  const files = view.state.excluded_by;
  return (
    <div className="ts-attach-panel ts-attach-panel--warn" role="status">
      <div className="ts-attach-panel__body">
        {files.includes("GitIgnore") && <strong>{t("attachments.warn.history")}</strong>}
        {files.includes("StIgnore") && <strong>{t("attachments.warn.delivery")}</strong>}
        <span>
          {t("attachments.warn.detail", {
            folder: view.state.target_folder,
            files: files.map((file) => (file === "GitIgnore" ? ".gitignore" : ".stignore")).join(", "),
          })}
        </span>
      </div>
      {onOpenRules !== null && files[0] !== undefined && (
        <button type="button" className="ts-btn" onClick={() => onOpenRules(files[0]!)}>
          {t("attachments.warn.openRules")}
        </button>
      )}
    </div>
  );
}
