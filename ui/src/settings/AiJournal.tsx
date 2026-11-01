"use client";

import { useCallback, useEffect, useState } from "react";
import type { JournalEntry } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";

/** Журнал отправленного (фича 050, макет G, H): всё, что ушло провайдеру, дословно. */
export function AiJournal({ client }: { client: VaultClient }): React.JSX.Element {
  const { t } = useI18n();
  const [entries, setEntries] = useState<JournalEntry[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);

  const load = useCallback(async () => {
    setEntries(await client.ai.journal());
  }, [client]);

  useEffect(() => {
    void load().catch(() => setEntries([]));
  }, [load]);

  if (entries === null) return <p role="status">{t("ai.journal.loading")}</p>;

  const current = entries.find((entry) => entry.id === selected) ?? entries[0] ?? null;

  const clear = async (): Promise<void> => {
    await client.ai.clearJournal();
    setConfirming(false);
    await load();
  };

  return (
    <section className="ts-ai__journal" aria-label={t("ai.journal.title")}>
      <h3 className="ts-srow__label">{t("ai.journal.title")}</h3>
      {current === null ? (
        <p className="ts-srow__hint">{t("ai.journal.empty")}</p>
      ) : (
        <>
          <table className="ts-ai__table">
            <thead>
              <tr>
                <th>{t("ai.journal.time")}</th>
                <th>{t("ai.model")}</th>
                <th>{t("ai.journal.volume")}</th>
                <th>{t("ai.journal.cost")}</th>
                <th>{t("ai.journal.result")}</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((entry) => {
                const tokens =
                  entry.input_tokens != null && entry.output_tokens != null
                    ? String(entry.input_tokens + entry.output_tokens)
                    : t("ai.journal.noData");
                return (
                  <tr
                    key={entry.id}
                    aria-selected={entry.id === current.id}
                    tabIndex={0}
                    onClick={() => setSelected(entry.id)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") setSelected(entry.id);
                    }}
                  >
                    <td>{new Date(entry.at).toLocaleTimeString()}</td>
                    <td>{entry.model}</td>
                    <td>{tokens}</td>
                    <td>
                      {entry.cost_usd != null
                        ? t("ai.costAbout", { cost: entry.cost_usd.toFixed(4) })
                        : t("ai.costUnknown")}
                    </td>
                    <td>
                      {entry.state === "Done" && t("ai.journal.done")}
                      {entry.state === "Interrupted" && t("ai.journal.interrupted")}
                      {entry.state === "Failed" &&
                        (entry.failure != null ? t(`ai.failure.${entry.failure.kind}`) : t("ai.failure.Unknown"))}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          <pre className="ts-ai__request">
            {t("ai.journal.sent", { endpoint: current.endpoint })}
            {"\n"}
            {current.request}
          </pre>
        </>
      )}
      <div className="ts-srow ts-srow--last">
        <span className="ts-srow__hint">{t("ai.journal.hint")}</span>
        <button
          type="button"
          className="ts-btn ts-btn--danger"
          disabled={entries.length === 0}
          onClick={() => setConfirming(true)}
        >
          {t("ai.journal.clear")}
        </button>
      </div>
      {confirming && (
        <div className="ts-replace__panel ts-replace__panel--warn" role="alertdialog">
          <span className="ts-replace__title">{t("ai.journal.clearConfirm")}</span>
          <div className="ts-replace__row">
            <button type="button" className="ts-btn ts-btn--danger" onClick={() => void clear()}>
              {t("ai.journal.clearDo")}
            </button>
            <button type="button" className="ts-btn ts-btn--quiet" onClick={() => setConfirming(false)}>
              {t("ai.cancel")}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
