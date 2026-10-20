"use client";

import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n/index.js";
import type { VaultClient } from "../vault-client.js";
import type {
  CitationScanView,
  CitingNote,
  LibrarySummary,
  MissingKey,
  Source,
  SourceHit,
  SourceHits,
} from "../wasm-core/typsidian_core.js";
import { Snippet, type RevealPlace } from "../note-panel/links/LinksMode.js";
import { authorsLine, problemDetail, problemText } from "./describe.js";

/** Строк в списке панели (contracts/frontend-sources.md § Панель). */
const PANEL_LIMIT = 200;

export interface SourcesPanelProps {
  client: VaultClient;
  summary: LibrarySummary | null;
  scan: CitationScanView | null;
  revision: number;
  readOnly: boolean;
  uncitedOnly: boolean;
  onUncitedOnly: (only: boolean) => void;
  onStartScan: () => void;
  onCancelScan: () => void;
  onReveal: (place: RevealPlace) => void;
  /** «Вставить @ключ»; `null` — заметка не открыта. */
  onInsert: ((source: Source) => void) | null;
  onCreateFile: () => Promise<string | null>;
  onRegisterFile: (path: string) => void;
  /** Показать источник по ключу (переход к определению 018): растущий `id`. */
  focus?: { id: number; key: string } | null;
}

/**
 * Режим «Источники» колонки заметки (фича 017, FR-005, макет 1a): поиск по
 * источникам, цитирующие заметки с переходом к цитате, ключи, которых нет в
 * файлах, и ошибки чтения файлов.
 */
export function SourcesPanel(props: SourcesPanelProps): React.JSX.Element {
  const { t } = useI18n();
  const { client, summary, scan, revision, uncitedOnly, onStartScan } = props;
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SourceHits | null>(null);
  const [selected, setSelected] = useState<Source | null>(null);
  const [citing, setCiting] = useState<CitingNote[] | null>(null);
  const [missing, setMissing] = useState<MissingKey[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);
  const asked = useRef(0);
  // Ключ, который надо выбрать, когда придёт выдача (018).
  const pendingKey = useRef<string | null>(null);
  const focusId = props.focus?.id ?? 0;
  const focusKey = props.focus?.key ?? null;
  useEffect(() => {
    if (focusKey === null) return;
    pendingKey.current = focusKey;
    setQuery(focusKey);
  }, [focusId, focusKey]);

  // Первый показ панели запускает проход индекса цитат (research.md §8).
  useEffect(() => {
    if (scan === null || scan.phase === "Idle") onStartScan();
  }, [scan, onStartScan]);

  useEffect(() => {
    const ticket = ++asked.current;
    void client.sources.search(query, PANEL_LIMIT, uncitedOnly).then((result) => {
      if (ticket !== asked.current) return;
      setHits(result);
      const wanted = pendingKey.current;
      const found = result.hits.find((hit) => hit.source.key === wanted);
      if (found !== undefined) {
        pendingKey.current = null;
        setSelected(found.source);
      }
    });
    void client.sources.missingKeys().then(setMissing);
  }, [client, query, uncitedOnly, revision, scan?.phase]);

  useEffect(() => {
    if (selected === null) {
      setCiting(null);
      return;
    }
    let alive = true;
    void client.sources.citing(selected.key).then((notes) => {
      if (alive) setCiting(notes);
    });
    return () => {
      alive = false;
    };
  }, [client, selected, revision]);

  if (summary !== null && summary.files.length === 0) {
    return (
      <div className="ts-sources">
        <div className="ts-sources__empty">
          <strong>{t("sources.empty.title")}</strong>
          {props.readOnly ? (
            <p>{t("sources.empty.readOnly")}</p>
          ) : (
            <button
              type="button"
              className="ts-btn ts-btn--accent"
              onClick={() => void props.onCreateFile().then(setMessage)}
            >
              {t("sources.empty.create")}
            </button>
          )}
          {summary.yaml_candidates.length > 0 && !props.readOnly ? (
            <>
              <button type="button" className="ts-btn" onClick={() => setPicking((open) => !open)}>
                {t("sources.empty.pick")}
              </button>
              {picking && (
                <ul className="ts-sources__candidates">
                  {summary.yaml_candidates.map((path) => (
                    <li key={path}>
                      <button type="button" className="ts-sources__candidate" onClick={() => props.onRegisterFile(path)}>
                        {path}
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </>
          ) : (
            <p className="ts-sources__hint">{t("sources.empty.hint")}</p>
          )}
          {message !== null && <p role="status">{message}</p>}
        </div>
      </div>
    );
  }

  const done = scan?.phase === "Done";
  const rows = hits?.hits ?? [];

  return (
    <div className="ts-sources">
      <div className="ts-sources__search">
        <input
          type="text"
          value={query}
          placeholder={t("sources.search.placeholder")}
          aria-label={t("sources.search.label")}
          spellCheck={false}
          onChange={(event) => setQuery(event.target.value)}
        />
        <label className="ts-sources__toggle">
          <input
            type="checkbox"
            checked={uncitedOnly}
            disabled={!done}
            onChange={(event) => props.onUncitedOnly(event.target.checked)}
          />
          {t("sources.uncitedOnly")}
        </label>
      </div>

      {scan?.phase === "Running" && (
        <p className="ts-sources__scan" role="status">
          {t("sources.scan.progress", { done: scan.processed, total: scan.total })} ·{" "}
          <button type="button" className="ts-sources__link" onClick={props.onCancelScan}>
            {t("sources.scan.cancel")}
          </button>
        </p>
      )}
      {scan?.phase === "Cancelled" && (
        <p className="ts-sources__scan" role="status">
          {t("sources.scan.stopped")} ·{" "}
          <button type="button" className="ts-sources__link" onClick={onStartScan}>
            {t("sources.scan.resume")}
          </button>
        </p>
      )}

      {summary !== null && summary.files.length > 0 && summary.source_count === 0 && (
        <p className="ts-sources__hint">
          {t("sources.empty.noEntries", { path: summary.files.map((file) => file.path).join(", ") })}
        </p>
      )}

      <ul className="ts-sources__list">
        {rows.map((hit) => {
          const isSelected =
            selected !== null && selected.key === hit.source.key && selected.file === hit.source.file;
          return (
            <li key={`${hit.source.file}\u0000${hit.source.key}`}>
              <SourceRow
                hit={hit}
                selected={isSelected}
                onSelect={() => setSelected(isSelected ? null : hit.source)}
              />
              {isSelected && (
                <SourceCard
                  hit={hit}
                  citing={citing}
                  onReveal={props.onReveal}
                  onInsert={props.onInsert}
                />
              )}
            </li>
          );
        })}
      </ul>
      {hits !== null && rows.length === 0 && <p className="ts-sources__hint">{t("sources.nothingFound")}</p>}
      {hits !== null && hits.total_matches > rows.length && (
        <p className="ts-sources__hint">{t("sources.shown", { shown: rows.length, total: hits.total_matches })}</p>
      )}

      {missing.length > 0 && (
        <section className="ts-sources__group">
          <h3>{t("sources.missing.title")}</h3>
          <ul>
            {missing.map((entry) => (
              <li key={entry.key}>
                <button
                  type="button"
                  className="ts-sources__missing"
                  onClick={() =>
                    props.onReveal({
                      path: entry.first_path,
                      line: entry.first.line,
                      column: entry.first.column,
                    })
                  }
                >
                  <span className="ts-sources__broken">@{entry.key}</span>
                  <span className="ts-tag ts-sources__tag--broken">{t("sources.tag.broken")}</span>
                  <span className="ts-sources__count">{t("sources.missing.notes", { count: entry.notes })}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      {summary !== null && summary.problems.length > 0 && (
        <section className="ts-sources__group ts-sources__group--problems">
          <h3>{t("sources.problems.title")}</h3>
          <ul>
            {summary.problems.map((problem, index) => {
              const detail = problemDetail(problem.kind);
              return (
                <li key={index} className="ts-sources__problem">
                  <span className="ts-sources__place">
                    {t("sources.problems.place", { file: problem.file, line: problem.line })}
                    {problem.key !== undefined && ` · ${problem.key}`}
                  </span>
                  <span>{problemText(t, problem.kind)}</span>
                  {detail !== null && <span className="ts-sources__detail">{detail}</span>}
                </li>
              );
            })}
          </ul>
        </section>
      )}
    </div>
  );
}

function SourceRow({
  hit,
  selected,
  onSelect,
}: {
  hit: SourceHit;
  selected: boolean;
  onSelect: () => void;
}): React.JSX.Element {
  const { t } = useI18n();
  const uncited = hit.cited === 0;
  const classes = ["ts-sources__row"];
  if (selected) classes.push("ts-sources__row--on");
  if (uncited) classes.push("ts-sources__row--uncited");
  return (
    <button type="button" className={classes.join(" ")} aria-pressed={selected} onClick={onSelect}>
      <span className="ts-sources__text">
        {[hit.source.authors[0], hit.source.title].filter((part) => part !== undefined && part !== "").join(" — ")}
      </span>
      <span className="ts-tag ts-sources__key">{hit.source.key}</span>
      {uncited && <span className="ts-tag ts-sources__tag--uncited">{t("sources.tag.uncited")}</span>}
    </button>
  );
}

function SourceCard({
  hit,
  citing,
  onReveal,
  onInsert,
}: {
  hit: SourceHit;
  citing: CitingNote[] | null;
  onReveal: (place: RevealPlace) => void;
  onInsert: ((source: Source) => void) | null;
}): React.JSX.Element {
  const { t } = useI18n();
  const { source } = hit;
  return (
    <div className="ts-sources__card">
      <span className="ts-sources__authors">{authorsLine(source)}</span>
      <strong className="ts-sources__title">{source.title}</strong>
      <span className="ts-sources__meta">
        {[source.container, source.year === undefined ? undefined : String(source.year)]
          .filter((part) => part !== undefined)
          .join(" · ")}
      </span>
      <span className="ts-sources__meta">
        {source.key}
        {hit.cited !== undefined && ` · ${t("sources.inText", { count: hit.cited })}`}
      </span>
      <span className="ts-sources__meta">{source.file}</span>
      {onInsert !== null && (
        <button type="button" className="ts-btn ts-btn--accent" onClick={() => onInsert(source)}>
          {t("sources.insertKey", { key: source.key })}
        </button>
      )}
      <h4>{t("sources.citing.title")}</h4>
      {citing !== null && citing.length === 0 && <p className="ts-sources__hint">{t("sources.citing.none")}</p>}
      {citing?.map((note) => (
        <div key={note.path} className="ts-links__group">
          <div className="ts-links__grouphead">
            <span className="ts-links__name">{note.name}</span>
          </div>
          {note.lines.map((line, index) => {
            const place = note.places[index];
            return (
              <div key={index} className="ts-links__row">
                <button
                  type="button"
                  className="ts-links__snip"
                  aria-label={t("links.rowLabel", { note: note.name, line: line.number })}
                  onClick={() =>
                    place !== undefined &&
                    onReveal({ path: note.path, line: place.line, column: place.column })
                  }
                >
                  <Snippet segments={line.segments} />
                </button>
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}

