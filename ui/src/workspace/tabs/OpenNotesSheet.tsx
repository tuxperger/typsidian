"use client";

import { useEffect, useRef } from "react";
import type { TabId, TabView, WorkspaceView } from "../../wasm-core/typsidian_core.js";
import { Icon } from "../../icons/Icons.js";
import { useI18n } from "../../i18n/index.js";
import { useDismissOnEscape } from "../useDismissOnEscape.js";

// Шторка «Открытые заметки» телефона (макет «Typsidian Вкладки» 1c): полосы
// вкладок на узком окне нет, список открывается из счётчика в шапке. Подачу
// `List` выбирает ядро по ширине окна (FR-024), а не таргет.

export interface OpenNotesSheetProps {
  view: WorkspaceView;
  onActivate(tab: TabId): void;
  onClose(tab: TabId): void;
  onCloseOthers(tab: TabId): void;
  /** Закрыть саму шторку. */
  onDismiss(): void;
}

export function OpenNotesSheet({
  view,
  onActivate,
  onClose,
  onCloseOthers,
  onDismiss,
}: OpenNotesSheetProps): React.JSX.Element {
  const { t } = useI18n();
  const panelRef = useRef<HTMLElement | null>(null);
  useDismissOnEscape(onDismiss);
  // Шторка модальна для скринридера: фокус уходит в неё при открытии.
  useEffect(() => panelRef.current?.focus(), []);

  const tabs: TabView[] = view.groups.flatMap((group) => group.tabs);
  const activeId = view.active?.tab;

  return (
    <div
      className="ts-open-notes__overlay"
      onClick={(event) => {
        if (event.target === event.currentTarget) onDismiss();
      }}
    >
      <aside
        ref={panelRef}
        className="ts-open-notes"
        role="dialog"
        aria-modal="true"
        aria-label={t("workspace.tabs.openNotes")}
        tabIndex={-1}
      >
        <button
          type="button"
          className="ts-open-notes__grab"
          aria-label={t("workspace.tabs.sheetClose")}
          onClick={onDismiss}
        />
        <div className="ts-open-notes__head">
          <span className="ts-eyebrow">{t("workspace.tabs.openNotes")}</span>
          <span className="ts-spacer" />
          <button
            type="button"
            className="ts-btn"
            disabled={tabs.length < 2 || activeId === undefined}
            onClick={() => {
              if (activeId !== undefined) onCloseOthers(activeId);
            }}
          >
            {t("workspace.tabs.closeOthers")}
          </button>
        </div>
        <ul className="ts-open-notes__list">
          {tabs.map((tab) => (
            <li key={tab.id} className="ts-open-notes__item" data-active={tab.id === activeId}>
              <button
                type="button"
                className="ts-open-notes__open"
                aria-current={tab.id === activeId ? "true" : undefined}
                onClick={() => {
                  onActivate(tab.id);
                  onDismiss();
                }}
              >
                <span className="ts-open-notes__title">{tab.title}</span>
                {tab.hint !== undefined && <span className="ts-tab__hint">{tab.hint}</span>}
                {tab.unsaved && (
                  <span
                    className="ts-tab__dot"
                    role="img"
                    aria-label={t("workspace.tabs.unsaved")}
                  />
                )}
                {tab.conflict && (
                  <span className="ts-tab__tag ts-tab__tag--warn">{t("workspace.tabs.conflict")}</span>
                )}
                {tab.kind === "Board" && (
                  <span className="ts-tab__tag">{t("workspace.tabs.board")}</span>
                )}
              </button>
              <button
                type="button"
                className="ts-open-notes__close"
                aria-label={t("workspace.tabs.closeNamed", { title: tab.title })}
                onClick={() => onClose(tab.id)}
              >
                <Icon name="x" size={12} />
              </button>
            </li>
          ))}
        </ul>
      </aside>
    </div>
  );
}
