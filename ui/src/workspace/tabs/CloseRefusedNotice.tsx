"use client";

import { useI18n } from "../../i18n/index.js";
import type { TabRefusal } from "./useWorkspaceTabs.js";

// Панель «Вкладка не закрыта» (макет «Typsidian Вкладки» 1c, FR-005). Вкладку,
// которую нельзя сохранить, ядро не закрывает; здесь названа причина.

export interface CloseRefusedNoticeProps {
  refusal: TabRefusal;
  onDismiss(): void;
}

export function CloseRefusedNotice({
  refusal,
  onDismiss,
}: CloseRefusedNoticeProps): React.JSX.Element {
  const { t } = useI18n();
  const text =
    refusal.reason === "Conflict"
      ? t("workspace.tabs.refusedConflict", { title: refusal.title })
      : refusal.detail !== null
        ? t("workspace.tabs.refusedUnsavedWhy", {
            title: refusal.title,
            reason: refusal.detail(t),
          })
        : t("workspace.tabs.refusedUnsaved", { title: refusal.title });
  return (
    <div className="ts-tab-refused" role="alert">
      <div className="ts-tab-refused__body">
        <div className="ts-tab-refused__title">{t("workspace.tabs.refused")}</div>
        <div className="ts-tab-refused__text">{text}</div>
      </div>
      <button type="button" className="ts-btn ts-btn--quiet" onClick={onDismiss}>
        {t("workspace.tabs.dismiss")}
      </button>
    </div>
  );
}
