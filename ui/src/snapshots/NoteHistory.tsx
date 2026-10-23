"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type { DiffLine, Snapshot, SnapshotEntry } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import type { OpenNoteEditors } from "../note-panel/links/openNoteEditors.js";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { snapshotClock } from "./when.js";

// Окно «История заметки» (фича 029, макет S1, S4). Список снимков, различия и
// возврат считает ядро (`snapshots`); версии git приходят из истории 011 и
// сравниваются там же. Здесь — выбор строки, свёртка неизменённых строк и показ.

export interface NoteHistoryProps {
  client: VaultClient;
  path: string;
  /** Правки открытой заметки идут через её редактор — одной единицей отмены. */
  editors: OpenNoteEditors;
  /** Версии git заметки (011); `null` — vault не под историей. */
  gitHistory: ((path: string) => Promise<Snapshot[]>) | null;
  onOpenGit: (path: string) => void;
  onOpenDeleted: () => void;
  /** Снимки перестали записываться — предупреждение S4. */
  failing: boolean;
  onClose: () => void;
}

type Row =
  | { kind: "local"; at: number; entry: SnapshotEntry }
  | { kind: "git"; at: number; snapshot: Snapshot };

/** Сколько неизменённых строк остаётся вокруг правки. */
const CONTEXT = 2;

type Shown =
  | { kind: "line"; line: DiffLine; number: number | null }
  | { kind: "gap"; count: number };

/** Строки различий со свёрткой длинных неизменённых кусков. */
function fold(diff: readonly DiffLine[]): Shown[] {
  const numbered: { line: DiffLine; number: number | null }[] = [];
  let current = 0;
  let old = 0;
  for (const line of diff) {
    if (line.side === "Local") {
      old += 1;
      numbered.push({ line, number: old });
    } else {
      current += 1;
      if (line.side === "Same") old += 1;
      numbered.push({ line, number: current });
    }
  }
  const near = numbered.map((_, index) =>
    numbered
      .slice(Math.max(0, index - CONTEXT), index + CONTEXT + 1)
      .some((other) => other.line.side !== "Same"),
  );
  const out: Shown[] = [];
  let gap = 0;
  numbered.forEach((entry, index) => {
    if (entry.line.side === "Same" && !near[index]) {
      gap += 1;
      return;
    }
    if (gap > 0) out.push({ kind: "gap", count: gap });
    gap = 0;
    out.push({ kind: "line", ...entry });
  });
  if (gap > 0) out.push({ kind: "gap", count: gap });
  return out;
}

export function NoteHistory({
  client,
  path,
  editors,
  gitHistory,
  onOpenGit,
  onOpenDeleted,
  failing,
  onClose,
}: NoteHistoryProps): React.JSX.Element {
  const { t, lang } = useI18n();
  useDismissOnEscape(onClose);
  const [entries, setEntries] = useState<SnapshotEntry[] | null>(null);
  const [git, setGit] = useState<Snapshot[]>([]);
  const [showLocal, setShowLocal] = useState(true);
  const [showGit, setShowGit] = useState(true);
  const [selected, setSelected] = useState<string | null>(null);
  const [diff, setDiff] = useState<DiffLine[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<"restored" | "failed" | "readOnly" | null>(null);
  const [restoredAt, setRestoredAt] = useState<number | null>(null);

  const load = useCallback(async () => {
    const history = await client.snapshots.history(path);
    setEntries(history.entries);
    setSelected((current) => current ?? history.entries[0]?.id ?? null);
  }, [client, path]);

  useEffect(() => {
    void load().catch(() => setEntries([]));
  }, [load]);

  useEffect(() => {
    if (gitHistory === null) return;
    let stale = false;
    void gitHistory(path)
      .then((found) => !stale && setGit(found))
      .catch(() => undefined);
    return () => {
      stale = true;
    };
  }, [gitHistory, path]);

  const rows = useMemo<Row[]>(() => {
    const local: Row[] = showLocal
      ? (entries ?? []).map((entry) => ({ kind: "local", at: entry.at_ms, entry }))
      : [];
    const versions: Row[] = showGit
      ? git.map((snapshot) => ({ kind: "git", at: Number(snapshot.time), snapshot }))
      : [];
    return [...local, ...versions].sort((a, b) => b.at - a.at);
  }, [entries, git, showLocal, showGit]);

  const chosen = rows.find((row) =>
    row.kind === "local" ? row.entry.id === selected : row.snapshot.id === selected,
  );

  const currentText = useCallback(
    async (): Promise<string> => editors.textOf(path) ?? (await client.read(path)),
    [client, editors, path],
  );

  useEffect(() => {
    if (chosen === undefined || chosen.kind !== "local") {
      setDiff(null);
      return;
    }
    let stale = false;
    void (async () => {
      const found = await client.snapshots.diff(chosen.entry.id, await currentText());
      if (!stale) setDiff(found);
    })().catch(() => !stale && setDiff([]));
    return () => {
      stale = true;
    };
  }, [chosen, client, currentText]);

  const { timeOf, dayOf, whenOf } = snapshotClock(t, lang);

  const restore = async (): Promise<void> => {
    if (chosen === undefined || chosen.kind !== "local" || busy) return;
    setBusy(true);
    try {
      const outcome = await client.snapshots.restore(path, chosen.entry.id, editors.textOf(path));
      switch (outcome.kind) {
        case "EditorEdit":
          setResult(editors.applyEdit(path, outcome.edit).ok ? "restored" : "failed");
          break;
        case "Written":
          setResult("restored");
          break;
        case "ReadOnly":
          setResult("readOnly");
          break;
        case "Missing":
        case "Failed":
          setResult("failed");
          break;
        default: {
          const exhaustive: never = outcome;
          return exhaustive;
        }
      }
      setRestoredAt(chosen.entry.at_ms);
      await load();
    } finally {
      setBusy(false);
    }
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    const index = rows.findIndex((row) => row === chosen);
    const idOf = (row: Row | undefined): string | null =>
      row === undefined ? null : row.kind === "local" ? row.entry.id : row.snapshot.id;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelected(idOf(rows[Math.min(rows.length - 1, index + 1)]) ?? selected);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelected(idOf(rows[Math.max(0, index - 1)]) ?? selected);
    } else if (event.key === "Enter") {
      event.preventDefault();
      void restore();
    }
  };

  let lastDay = "";
  const removed = diff?.filter((line) => line.side === "Local").length ?? 0;
  const added = diff?.filter((line) => line.side === "External").length ?? 0;

  return (
    <div className="ts-overlay" role="presentation">
      <section
        className="ts-search ts-history"
        aria-label={t("snapshots.title")}
        onKeyDown={onKeyDown}
        tabIndex={-1}
      >
        <div className="ts-search__query">
          <span className="ts-history__title">{t("snapshots.title")}</span>
          <span className="ts-search__hit-meta ts-history__path">{path}</span>
          <span className="ts-spacer" />
          <button
            type="button"
            className="ts-chip"
            aria-pressed={showLocal}
            onClick={() => setShowLocal((on) => !on)}
          >
            {t("snapshots.kindLocal")}
          </button>
          {gitHistory !== null && (
            <button
              type="button"
              className="ts-chip"
              aria-pressed={showGit}
              onClick={() => setShowGit((on) => !on)}
            >
              {t("snapshots.kindGit")}
            </button>
          )}
          <button type="button" className="ts-btn ts-btn--quiet" onClick={onOpenDeleted}>
            {t("snapshots.deletedTitle")}
          </button>
          <button
            type="button"
            className="ts-btn ts-btn--ghost"
            onClick={onClose}
            aria-label={t("snapshots.close")}
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        {failing && (
          <div className="ts-replace__panel ts-replace__panel--warn" role="status">
            <span className="ts-replace__title">{t("snapshots.failing")}</span>
            <span className="ts-replace__meta">{t("snapshots.failingHint")}</span>
          </div>
        )}

        {entries !== null && rows.length === 0 ? (
          <p className="ts-search__empty">{t("snapshots.empty")}</p>
        ) : (
          <div className="ts-history__body">
            <div className="ts-history__list" role="listbox" aria-label={t("snapshots.title")}>
              <div className="ts-history__snap ts-history__snap--current">
                <span className="ts-history__time">{t("snapshots.current")}</span>
                <span className="ts-history__why">{t("snapshots.currentText")}</span>
              </div>
              {rows.map((row) => {
                const day = dayOf(row.at);
                const head = day !== lastDay ? day : null;
                lastDay = day;
                const id = row.kind === "local" ? row.entry.id : row.snapshot.id;
                return (
                  <div key={`${row.kind}:${id}`}>
                    {head !== null && <div className="ts-history__day">{head}</div>}
                    <button
                      type="button"
                      role="option"
                      className="ts-history__snap"
                      aria-selected={id === selected}
                      onClick={() => setSelected(id)}
                    >
                      <span className="ts-history__time">{timeOf(row.at)}</span>
                      <span className="ts-history__why">
                        {row.kind === "local"
                          ? t(`snapshots.reason.${row.entry.reason}`)
                          : t("snapshots.gitReason")}
                        {row.kind === "git" && (
                          <span className="ts-history__git">{t("snapshots.kindGit")}</span>
                        )}
                      </span>
                      {row.kind === "local" && (
                        <span className="ts-history__delta">
                          <span className="ts-history__added">+{row.entry.added}</span>{" "}
                          <span className="ts-history__removed">−{row.entry.removed}</span>
                        </span>
                      )}
                    </button>
                  </div>
                );
              })}
            </div>

            <div className="ts-history__diff">
              {chosen === undefined && <p className="ts-search__empty">{t("snapshots.pick")}</p>}
              {chosen?.kind === "git" && (
                <div className="ts-history__git-pane">
                  <p className="ts-search__empty">{t("snapshots.gitHint")}</p>
                  <button type="button" className="ts-btn" onClick={() => onOpenGit(path)}>
                    {t("snapshots.openGit")}
                  </button>
                </div>
              )}
              {chosen?.kind === "local" && (
                <>
                  <div className="ts-history__dhead">
                    <span>{t("snapshots.compare", { when: whenOf(chosen.entry.at_ms) })}</span>
                    <span className="ts-spacer" />
                    {diff !== null && <span>{t("snapshots.compareSummary", { removed, added })}</span>}
                  </div>
                  {diff !== null && removed + added === 0 && (
                    <p className="ts-search__empty">{t("snapshots.same")}</p>
                  )}
                  {diff !== null && removed + added > 0 && (
                    <div className="ts-history__lines">
                      {fold(diff).map((shown, index) =>
                        shown.kind === "gap" ? (
                          <div key={index} className="ts-history__line ts-history__line--gap">
                            <span />
                            <span />
                            <span>{t("snapshots.unchanged", { count: shown.count })}</span>
                          </div>
                        ) : (
                          <div
                            key={index}
                            className={`ts-history__line ts-history__line--${shown.line.side}`}
                          >
                            <span className="ts-history__n">{shown.number}</span>
                            <span>
                              {shown.line.side === "Local"
                                ? "−"
                                : shown.line.side === "External"
                                  ? "+"
                                  : ""}
                            </span>
                            <span>{shown.line.text === "" ? " " : shown.line.text}</span>
                          </div>
                        ),
                      )}
                    </div>
                  )}
                </>
              )}
            </div>
          </div>
        )}

        {result !== null && (
          <div
            className={
              result === "restored"
                ? "ts-replace__panel"
                : "ts-replace__panel ts-replace__panel--err"
            }
            role="status"
          >
            <span className="ts-replace__title">
              {result === "restored"
                ? t("snapshots.restored", { when: whenOf(restoredAt ?? Date.now()) })
                : result === "readOnly"
                  ? t("snapshots.readOnly")
                  : t("snapshots.restoreFailed")}
            </span>
            {result === "restored" && (
              <span className="ts-replace__meta">{t("snapshots.restoredHint")}</span>
            )}
          </div>
        )}

        <div className="ts-search__hints ts-replace__foot">
          <span>{t("snapshots.hints")}</span>
          <span className="ts-spacer" />
          <button
            type="button"
            className="ts-btn ts-btn--accent"
            disabled={chosen?.kind !== "local" || busy}
            onClick={() => void restore()}
          >
            {t("snapshots.restore")}
          </button>
        </div>
      </section>
    </div>
  );
}
