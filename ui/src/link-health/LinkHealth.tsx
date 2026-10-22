"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import type { BrokenTarget, Note, Orphan } from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import { useI18n, type Translate } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import type { FixReport, LinkHealthState } from "./useLinkHealth.js";

// Экран «Состояние ссылок» (фича 028, макет L1–L2): вкладки «Битые ссылки» и
// «Без входящих». Списки, числа и куски строк — ответ ядра; здесь разметка,
// фильтр по строке и выбор заметки для перенаправления.

export type LinkHealthTab = "broken" | "orphans";

export interface LinkHealthProps {
  state: LinkHealthState;
  /** Заметки и доски vault — для выбора цели перенаправления. */
  notes: readonly Note[];
  readOnly: boolean;
  initialTab?: LinkHealthTab;
  onOpenPlace: (path: string, line: number) => void;
  onClose: () => void;
}

/** Сколько вариантов показывает выбор заметки. */
const PICKER_LIMIT = 8;

const MS_PER_MINUTE = 60_000;

function kindLabel(t: Translate, target: BrokenTarget): string {
  switch (target.kind) {
    case "Board":
      return t("linkHealth.kind.board");
    case "Attachment":
      return t("linkHealth.kind.attachment");
    default:
      return t("linkHealth.kind.note");
  }
}

function folderOf(path: string): string {
  const cut = path.lastIndexOf("/");
  return cut < 0 ? "" : path.slice(0, cut);
}

export function LinkHealth({
  state,
  notes,
  readOnly,
  initialTab = "broken",
  onOpenPlace,
  onClose,
}: LinkHealthProps): React.JSX.Element {
  const { t, formatRelative, formatBytes } = useI18n();
  const [tab, setTab] = useState<LinkHealthTab>(initialTab);
  const [filter, setFilter] = useState("");
  const [hideEntries, setHideEntries] = useState(true);
  const [picking, setPicking] = useState<string | null>(null);
  useDismissOnEscape(onClose);

  const inputRef = useRef<HTMLInputElement | null>(null);
  useEffect(() => inputRef.current?.focus(), [tab]);

  const health = state.health;
  const needle = filter.trim().toLowerCase();

  const broken = useMemo(
    () =>
      (health?.broken ?? []).filter(
        (target) =>
          needle === "" ||
          target.target.toLowerCase().includes(needle) ||
          target.notes.some((note) => note.path.toLowerCase().includes(needle)),
      ),
    [health, needle],
  );

  const orphans = useMemo(
    () =>
      (health?.orphans ?? []).filter(
        (orphan) =>
          (!hideEntries || orphan.kind !== "Entry") &&
          (needle === "" || orphan.path.toLowerCase().includes(needle)),
      ),
    [health, hideEntries, needle],
  );

  const open = (path: string, line: number) => {
    onOpenPlace(path, line);
    onClose();
  };

  const age = (orphan: Orphan): string => {
    if (orphan.modified_ms <= 0) return "";
    const minutes = Math.max(1, Math.round((Date.now() - orphan.modified_ms) / MS_PER_MINUTE));
    if (minutes < 60) return formatRelative(-minutes, "minute");
    if (minutes < 60 * 24) return formatRelative(-Math.round(minutes / 60), "hour");
    return formatRelative(-Math.round(minutes / (60 * 24)), "day");
  };

  return (
    <div className="ts-overlay" role="presentation">
      <section className="ts-search ts-health" aria-label={t("linkHealth.title")}>
        <div className="ts-search__query">
          <Icon name="link" size={16} />
          <input
            ref={inputRef}
            className="ts-search__input"
            value={filter}
            placeholder={
              tab === "broken" ? t("linkHealth.filterBroken") : t("linkHealth.filterOrphans")
            }
            onChange={(event) => setFilter(event.target.value)}
            aria-label={t("linkHealth.filterLabel")}
          />
          {tab === "orphans" && (
            <button
              type="button"
              className="ts-chip"
              aria-pressed={hideEntries}
              onClick={() => setHideEntries((on) => !on)}
              title={t("linkHealth.hideEntriesHint")}
            >
              {t("linkHealth.hideEntries")}
            </button>
          )}
          <span className="ts-search__sep" />
          <span className="ts-search__count">
            {health === null
              ? t("linkHealth.loading")
              : tab === "broken"
                ? t("linkHealth.brokenCount", {
                    links: health.broken_links,
                    targets: health.broken.length,
                  })
                : t("links.notesCount", { count: orphans.length })}
          </span>
          <button
            type="button"
            className="ts-btn ts-btn--ghost"
            onClick={onClose}
            aria-label={t("linkHealth.close")}
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        <div className="ts-health__tabs" role="tablist">
          <button
            type="button"
            role="tab"
            className="ts-health__tab"
            aria-selected={tab === "broken"}
            onClick={() => setTab("broken")}
          >
            {t("linkHealth.tabBroken")}
            <span className="ts-health__tab-n">{health?.broken_links ?? "…"}</span>
          </button>
          <button
            type="button"
            role="tab"
            className="ts-health__tab"
            aria-selected={tab === "orphans"}
            onClick={() => setTab("orphans")}
          >
            {t("linkHealth.tabOrphans")}
            <span className="ts-health__tab-n">{health?.orphans.length ?? "…"}</span>
          </button>
        </div>

        <div className="ts-search__results">
          {tab === "broken" && health !== null && health.broken.length === 0 && (
            <p className="ts-search__empty">{t("linkHealth.noBroken")}</p>
          )}
          {tab === "broken" &&
            broken.map((target) => (
              <BrokenGroup
                key={target.target}
                target={target}
                state={state}
                notes={notes}
                readOnly={readOnly}
                picking={picking === target.target}
                onPick={(on) => setPicking(on ? target.target : null)}
                onOpen={open}
              />
            ))}

          {tab === "orphans" && health !== null && orphans.length === 0 && (
            <p className="ts-search__empty">{t("linkHealth.noOrphans")}</p>
          )}
          {tab === "orphans" &&
            orphans.map((orphan) => (
              <div key={orphan.path} className="ts-health__orphan">
                <span className="ts-health__orphan-name">
                  {orphan.title}
                  {orphan.kind === "Entry" && (
                    <span className="ts-health__badge">{t("linkHealth.entry")}</span>
                  )}
                </span>
                <span className="ts-search__hit-meta">
                  {orphan.folder === "" ? t("search.vaultRoot") : `${orphan.folder}/`}
                </span>
                <span className="ts-search__hit-meta">{age(orphan)}</span>
                <span className="ts-search__hit-meta">{formatBytes(Number(orphan.size))}</span>
                <span className="ts-health__acts">
                  <button
                    type="button"
                    className="ts-btn ts-btn--quiet"
                    onClick={() => open(orphan.path, 1)}
                  >
                    {t("linkHealth.open")}
                  </button>
                  {orphan.kind === "Note" && (
                    <button
                      type="button"
                      className="ts-btn ts-btn--quiet"
                      disabled={readOnly}
                      title={t("linkHealth.intentionalHint")}
                      onClick={() => void state.setIntentional(orphan.path, true)}
                    >
                      {t("linkHealth.intentional")}
                    </button>
                  )}
                </span>
              </div>
            ))}
        </div>

        {state.report !== null && <ReportPanel report={state.report} state={state} />}

        <div className="ts-search__hints">
          {tab === "broken" ? (
            <span>{t("linkHealth.hintBroken")}</span>
          ) : (
            <>
              <span>{t("linkHealth.hintOrphans")}</span>
              <span className="ts-spacer" />
              {health !== null && health.intentional > 0 && (
                <span>{t("linkHealth.hiddenIntentional", { count: health.intentional })}</span>
              )}
            </>
          )}
        </div>
      </section>
    </div>
  );
}

interface BrokenGroupProps {
  target: BrokenTarget;
  state: LinkHealthState;
  notes: readonly Note[];
  readOnly: boolean;
  picking: boolean;
  onPick: (on: boolean) => void;
  onOpen: (path: string, line: number) => void;
}

function BrokenGroup({
  target,
  state,
  notes,
  readOnly,
  picking,
  onPick,
  onOpen,
}: BrokenGroupProps): React.JSX.Element {
  const { t } = useI18n();
  const attachment = target.kind === "Attachment";
  const off = readOnly || state.busy;
  return (
    <div className="ts-health__target">
      <div className="ts-health__head">
        <span className="ts-health__name">{attachment ? target.target.split("/").pop() : target.title}</span>
        <span className="ts-health__kind">{kindLabel(t, target)}</span>
        <span className="ts-search__hit-meta">
          {attachment
            ? t("linkHealth.embeds", { count: target.links })
            : t("links.count", { count: target.links })}{" "}
          · {t("links.notesCount", { count: target.sources })}
        </span>
        <span className="ts-health__acts">
          {!attachment && (
            <>
              <button
                type="button"
                className="ts-btn"
                disabled={off}
                onClick={() => void state.create(target)}
              >
                {target.kind === "Board" ? t("linkHealth.createBoard") : t("linkHealth.createNote")}
              </button>
              <button
                type="button"
                className="ts-btn ts-btn--accent"
                disabled={off}
                aria-expanded={picking}
                onClick={() => onPick(!picking)}
              >
                {t("linkHealth.redirect")}
              </button>
            </>
          )}
          <button
            type="button"
            className="ts-btn ts-btn--quiet"
            disabled={off}
            onClick={() => void state.fixGroup(target, { kind: "Unlink" })}
          >
            {attachment ? t("linkHealth.removeEmbeds") : t("linkHealth.unlinkAll")}
          </button>
        </span>
      </div>
      {target.sites.map((site) => (
        <div key={`${site.source}#${site.index}`} className="ts-health__site">
          <span className="ts-health__src" title={site.source}>
            {site.source.replace(/\.typ$/, "")}
          </span>
          <button
            type="button"
            className="ts-replace__text"
            onClick={() => onOpen(site.source, site.line)}
            aria-label={t("links.rowLabel", { note: site.source_title, line: site.line })}
          >
            {site.pieces.map((piece, index) =>
              piece.kind === "Removed" ? (
                <span key={index} className="ts-health__link">
                  {piece.text}
                </span>
              ) : (
                <span key={index}>{piece.text}</span>
              ),
            )}
          </button>
          <span className="ts-health__line">
            {t("linkHealth.line", { line: site.line })}
            {!attachment && (
              <button
                type="button"
                className="ts-btn ts-btn--ghost ts-health__x"
                disabled={off}
                title={t("linkHealth.unlinkOne")}
                aria-label={t("linkHealth.unlinkOne")}
                onClick={() => void state.unlinkSite(target, site)}
              >
                <Icon name="x" size={10} />
              </button>
            )}
          </span>
        </div>
      ))}
      {target.links > target.sites.length && (
        <p className="ts-search__hit-meta ts-health__more">
          {t("linkHealth.moreSites", { count: target.links - target.sites.length })}
        </p>
      )}
      {picking && (
        <NotePicker
          notes={notes}
          board={target.kind === "Board"}
          onPick={(to) => {
            onPick(false);
            void state.fixGroup(target, { kind: "Redirect", to });
          }}
          onCancel={() => onPick(false)}
        />
      )}
    </div>
  );
}

function NotePicker({
  notes,
  board,
  onPick,
  onCancel,
}: {
  notes: readonly Note[];
  board: boolean;
  onPick: (path: string) => void;
  onCancel: () => void;
}): React.JSX.Element {
  const { t } = useI18n();
  const [text, setText] = useState("");
  const [active, setActive] = useState(0);
  const options = useMemo(() => {
    const needle = text.trim().toLowerCase();
    return notes
      .filter((note) => (board ? note.kind === "Board" : note.kind === "Note"))
      .filter((note) => needle === "" || note.name.toLowerCase().includes(needle))
      .slice(0, PICKER_LIMIT);
  }, [notes, board, text]);

  return (
    <div className="ts-health__picker">
      <div className="ts-health__picker-field">
        <Icon name="search" size={12} />
        <input
          autoFocus
          className="ts-search__input"
          value={text}
          placeholder={t("linkHealth.pickPlaceholder")}
          aria-label={t("linkHealth.pickLabel")}
          onChange={(event) => {
            setText(event.target.value);
            setActive(0);
          }}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") {
              event.preventDefault();
              setActive((index) => Math.min(options.length - 1, index + 1));
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              setActive((index) => Math.max(0, index - 1));
            } else if (event.key === "Enter") {
              event.preventDefault();
              const chosen = options[active];
              if (chosen) onPick(chosen.path);
            } else if (event.key === "Escape") {
              event.preventDefault();
              event.stopPropagation();
              onCancel();
            }
          }}
        />
      </div>
      {options.length === 0 && <p className="ts-search__empty">{t("linkHealth.pickNothing")}</p>}
      {options.map((note, index) => (
        <button
          key={note.path}
          type="button"
          className="ts-health__option"
          aria-selected={index === active}
          onMouseEnter={() => setActive(index)}
          onClick={() => onPick(note.path)}
        >
          {note.name.replace(/\.(typ|excalidraw)$/, "")}
          <span className="ts-health__option-path">{folderOf(note.path)}/</span>
        </button>
      ))}
    </div>
  );
}

function ReportPanel({
  report,
  state,
}: {
  report: FixReport;
  state: LinkHealthState;
}): React.JSX.Element {
  const { t } = useI18n();
  if (report.readOnly) {
    return (
      <div className="ts-replace__panel ts-replace__panel--err" role="alert">
        <span className="ts-replace__title">{t("linkHealth.readOnly")}</span>
      </div>
    );
  }
  const title =
    report.fix === "Create"
      ? t("linkHealth.created", { name: report.to ?? "" })
      : report.links === null
        ? t("linkHealth.fixedNotes", { count: report.notes.length })
        : report.fix === "Redirect"
          ? t("linkHealth.redirected", { count: report.links, name: report.to ?? "" })
          : t("linkHealth.unlinked", { count: report.links });
  return (
    <div
      className={
        report.skipped.length > 0 ? "ts-replace__panel ts-replace__panel--warn" : "ts-replace__panel"
      }
      role="status"
    >
      <div className="ts-replace__row">
        <span className="ts-replace__title">{title}</span>
        <span className="ts-spacer" />
        {state.canUndo && (
          <button type="button" className="ts-btn" onClick={() => void state.undo()}>
            {t("linkHealth.undo")}
          </button>
        )}
        <button
          type="button"
          className="ts-btn ts-btn--ghost"
          aria-label={t("linkHealth.dismiss")}
          onClick={state.dismissReport}
        >
          <Icon name="x" size={12} />
        </button>
      </div>
      {report.notes.length > 0 && (
        <span className="ts-replace__meta">{report.notes.join(" · ")}</span>
      )}
      {report.skipped.length > 0 && (
        <span className="ts-replace__meta ts-replace__meta--err">
          {t("linkHealth.skipped", { names: report.skipped.join(", ") })}
        </span>
      )}
    </div>
  );
}
