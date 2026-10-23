"use client";

import { useCallback, useEffect, useState } from "react";
import type { DeletedNote } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { snapshotClock } from "./when.js";

// «Недавно удалённые» (фича 029, макет S2). Что удалено, из какого снимка и
// куда восстанавливать — решает ядро (`snapshots::recover_note`); занятый путь
// приходит ответом с предложенным именем, его можно поправить.

export interface RecentlyDeletedProps {
  client: VaultClient;
  /** Срок хранения, дней — для подписи. */
  keepDays: number;
  readOnly: boolean;
  onOpenHistory: (path: string) => void;
  /** Заметка восстановлена: дерево перечитывается, заметка открывается. */
  onRecovered: (path: string) => void;
  onClose: () => void;
}

export function RecentlyDeleted({
  client,
  keepDays,
  readOnly,
  onOpenHistory,
  onRecovered,
  onClose,
}: RecentlyDeletedProps): React.JSX.Element {
  const { t, lang } = useI18n();
  useDismissOnEscape(onClose);
  const [notes, setNotes] = useState<DeletedNote[] | null>(null);
  const [renaming, setRenaming] = useState<{ path: string; name: string } | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const load = useCallback(async () => {
    setNotes(await client.snapshots.deleted());
  }, [client]);

  useEffect(() => {
    void load().catch(() => setNotes([]));
  }, [load]);

  const recover = async (path: string, asPath: string | null): Promise<void> => {
    const outcome = await client.snapshots.recover(path, asPath);
    switch (outcome.kind) {
      case "Recovered":
        setRenaming(null);
        setMessage(t("snapshots.recovered", { path: outcome.path }));
        onRecovered(outcome.path);
        await load();
        break;
      case "Occupied":
        setRenaming({ path, name: outcome.suggested });
        break;
      case "ReadOnly":
        setMessage(t("snapshots.readOnly"));
        break;
      case "Missing":
      case "Failed":
        setMessage(t("snapshots.restoreFailed"));
        break;
      default: {
        const exhaustive: never = outcome;
        return exhaustive;
      }
    }
  };

  const when = snapshotClock(t, lang).whenOf;

  return (
    <div className="ts-overlay" role="presentation">
      <section className="ts-search ts-deleted" aria-label={t("snapshots.deletedTitle")}>
        <div className="ts-search__query">
          <span className="ts-history__title">{t("snapshots.deletedTitle")}</span>
          <span className="ts-spacer" />
          {notes !== null && (
            <span className="ts-search__count">
              {t("snapshots.deletedCount", { count: notes.length, days: keepDays })}
            </span>
          )}
          <button
            type="button"
            className="ts-btn ts-btn--ghost"
            onClick={onClose}
            aria-label={t("snapshots.close")}
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        <div className="ts-search__results">
          {notes !== null && notes.length === 0 && (
            <p className="ts-search__empty">{t("snapshots.noDeleted")}</p>
          )}
          {notes?.map((note) => (
            <div key={note.path}>
              <div className="ts-deleted__row">
                <span className="ts-deleted__name">
                  {note.title}
                  <span className="ts-search__hit-meta">
                    {note.folder === "" ? t("search.vaultRoot") : `${note.folder}/`}
                  </span>
                </span>
                <span className="ts-search__hit-meta">
                  {t("snapshots.deletedAt", { when: when(note.deleted_at_ms) })}
                </span>
                <span className="ts-search__hit-meta">
                  {t("snapshots.snapshotCount", { count: note.snapshots })}
                </span>
                <span className="ts-health__acts">
                  <button
                    type="button"
                    className="ts-btn ts-btn--quiet"
                    onClick={() => onOpenHistory(note.path)}
                  >
                    {t("snapshots.history")}
                  </button>
                  <button
                    type="button"
                    className="ts-btn"
                    disabled={readOnly}
                    onClick={() => void recover(note.path, null)}
                  >
                    {t("snapshots.recover")}
                  </button>
                </span>
              </div>
              {renaming?.path === note.path && (
                <form
                  className="ts-deleted__rename"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void recover(note.path, renaming.name);
                  }}
                >
                  <label className="ts-replace__meta" htmlFor="ts-deleted-name">
                    {t("snapshots.occupied", { path: note.path })}
                  </label>
                  <input
                    id="ts-deleted-name"
                    autoFocus
                    className="ts-deleted__field"
                    value={renaming.name}
                    onChange={(event) => setRenaming({ path: note.path, name: event.target.value })}
                  />
                  <button type="submit" className="ts-btn ts-btn--accent">
                    {t("snapshots.recover")}
                  </button>
                  <button
                    type="button"
                    className="ts-btn ts-btn--quiet"
                    onClick={() => setRenaming(null)}
                  >
                    {t("snapshots.cancel")}
                  </button>
                </form>
              )}
            </div>
          ))}
        </div>

        {message !== null && (
          <div className="ts-replace__panel" role="status">
            <span className="ts-replace__title">{message}</span>
          </div>
        )}
      </section>
    </div>
  );
}
