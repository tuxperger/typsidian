"use client";

import { useState } from "react";
import type { LineSegment, SiteRow, SourceGroup } from "../../wasm-core/typsidian_core.js";
import { useI18n } from "../../i18n/index.js";
import { mentionKey, LINKS_PAGE, type NoteLinks } from "./useNoteLinks.js";
import { outcomePhrase, problemPhrase, unavailablePhrase } from "./describe.js";

// Режим «Связи» колонки заметки (фича 013, макет «Typsidian Связи» 1a–1c):
// «Обратные ссылки» и раскрываемые «Упоминания». Всё, что здесь есть, — разметка
// ответов ядра; что считать ссылкой и как строить фрагмент, решило ядро.

/** Место, к которому ведёт строка: то же, что у перехода к ошибке предпросмотра. */
export interface RevealPlace {
  path: string;
  line: number;
  column: number;
}

export interface LinksModeProps {
  /** Открыта ли заметка или доска. */
  hasTarget: boolean;
  links: NoteLinks;
  /** Vault открыт только на чтение: «Связать» недоступна, причина названа (FR-018). */
  readOnly: boolean;
  mentionsExpanded: boolean;
  onMentionsExpanded: (expanded: boolean) => void;
  /** Перейти к месту ссылки; в шторке вызывающий ещё и закроет её (FR-003). */
  onReveal: (place: RevealPlace) => void;
}

export function LinksMode({
  hasTarget,
  links,
  readOnly,
  mentionsExpanded,
  onMentionsExpanded,
  onReveal,
}: LinksModeProps): React.JSX.Element {
  const { t, formatNumber } = useI18n();
  const [backlinksOpen, setBacklinksOpen] = useState(true);
  // Последняя строка, по которой перешли, — подсвечена как выбранная (макет 1a).
  const [activeKey, setActiveKey] = useState<string | null>(null);
  // Сколько групп упоминаний показано: ядро отдаёт все, порции — на стороне
  // показа (FR-005, research.md §16).
  const [mentionsShown, setMentionsShown] = useState(LINKS_PAGE);

  if (!hasTarget) {
    return <p className="ts-links__empty">{t("notePanel.openNote")}</p>;
  }

  const { backlinks, mentions } = links;

  const reveal = (key: string, group: SourceGroup, row: SiteRow): void => {
    setActiveKey(key);
    onReveal({ path: group.path, line: row.place.line, column: row.place.column });
  };

  const renderRow = (group: SourceGroup, row: SiteRow, withLink: boolean): React.JSX.Element => {
    const key = mentionKey(group.path, row);
    const active = key === activeKey;
    const outcome = links.outcomes.get(key);
    // Несостоявшаяся «Связать» объясняется под строкой; `Stale` — на месте
    // группы, а не строки.
    const phrase =
      outcome === undefined || outcome.kind === "Stale" ? null : outcomePhrase(outcome);
    return (
      <div key={key}>
        <div className={active ? "ts-links__row ts-links__row--active" : "ts-links__row"}>
          <button
            type="button"
            className="ts-links__snip"
            aria-label={t("links.rowLabel", { note: group.title, line: row.place.line })}
            onClick={() => reveal(key, group, row)}
          >
            <Snippet segments={row.snippet} />
          </button>
          {withLink && (
            <button
              type="button"
              className="ts-btn ts-links__link"
              aria-label={t("links.mentions.linkLabel", {
                note: group.title,
                line: row.place.line,
              })}
              disabled={readOnly || links.linkingKey !== null}
              onClick={() => void links.link(group, row)}
            >
              {t("links.mentions.link")}
            </button>
          )}
        </div>
        {phrase !== null && <p className="ts-links__note ts-links__note--indent">{phrase(t)}</p>}
      </div>
    );
  };

  const renderGroup = (
    group: SourceGroup,
    withLink: boolean,
    staleNote: boolean,
  ): React.JSX.Element => (
    <div key={group.path} className="ts-links__group">
      <div className="ts-links__grouphead">
        <span className="ts-links__name">{group.title}</span>
        <span className="ts-spacer" />
        {!group.same_folder && (
          <span className="ts-links__folder">
            {group.folder === "" ? "/" : `${group.folder}/`}
          </span>
        )}
        {staleNote && (
          <span className="ts-notepanel__tag ts-notepanel__tag--warn">
            {t("links.mentions.stale")}
          </span>
        )}
        {/* У источника с `problem` числа ссылок нет: файл не прочитан (research.md §16). */}
        {group.problem === undefined && group.rows.length > 0 && (
          <span className="ts-notepanel__tag" aria-label={t("links.count", { count: group.rows.length })}>
            {formatNumber(group.rows.length)}
          </span>
        )}
      </div>
      {staleNote && (
        <p className="ts-links__note ts-links__note--warn ts-links__note--indent">
          {t("links.mentions.staleNote")}
        </p>
      )}
      {group.problem !== undefined ? (
        <p className="ts-links__note ts-links__note--muted ts-links__note--indent">
          {problemPhrase(group.problem)(t)}
        </p>
      ) : (
        group.rows.map((row) => renderRow(group, row, withLink))
      )}
    </div>
  );

  // ── Обратные ссылки ────────────────────────────────────────────────────

  const backlinksBody = ((): React.ReactNode => {
    switch (backlinks.phase) {
      case "idle":
        return null;
      case "collecting":
      case "loading":
        return <Busy text={t("links.backlinks.collecting")} />;
      case "failed":
        return <p className="ts-links__note">{t("links.backlinks.failed")}</p>;
      case "ready": {
        if (backlinks.groups.length === 0) {
          return (
            <div className="ts-links__empty">
              <span>{t("links.backlinks.empty")}</span>
              <span>
                {t("links.backlinks.emptyExample")}{" "}
                <code className="ts-links__code">{`[[${links.title ?? ""}]]`}</code>
              </span>
            </div>
          );
        }
        const left = Math.min(LINKS_PAGE, backlinks.totalSources - backlinks.groups.length);
        return (
          <>
            {backlinks.groups.map((group) => renderGroup(group, false, false))}
            {backlinks.hasMore && (
              <>
                <p className="ts-links__note">
                  {t("links.partial", {
                    shown: backlinks.groups.length,
                    total: backlinks.totalSources,
                  })}
                </p>
                <div className="ts-links__more">
                  <button
                    type="button"
                    className="ts-btn"
                    disabled={backlinks.loadingMore}
                    onClick={links.showMore}
                  >
                    {t("links.more", { count: Math.max(left, 1) })}
                  </button>
                </div>
              </>
            )}
          </>
        );
      }
    }
  })();

  const count = links.backlinkCount;

  // ── Упоминания ─────────────────────────────────────────────────────────

  const mentionGroups =
    mentions.phase === "ready" || mentions.phase === "scanning" ? mentions.groups : [];
  // Заметка, где «Связать» не нашло места, остаётся в списке с тегом, даже если
  // после обновления упоминаний в ней уже нет.
  const listedPaths = new Set(mentionGroups.map((group) => group.path));
  const orphanStale = [...links.staleSources.values()].filter(
    (group) => !listedPaths.has(group.path),
  );

  const mentionsBody = ((): React.ReactNode => {
    switch (mentions.phase) {
      case "collapsed":
        return null;
      case "failed":
        return <p className="ts-links__note">{t("links.mentions.failed")}</p>;
      case "unavailable": {
        const phrase = unavailablePhrase(mentions.reason);
        return phrase === null ? null : <p className="ts-links__empty">{phrase(t)}</p>;
      }
      case "scanning":
      case "ready": {
        const visible = mentionGroups.slice(0, mentionsShown);
        const hidden = mentionGroups.length - visible.length;
        return (
          <>
            {mentions.phase === "scanning" && (
              <Busy
                text={t("links.mentions.scanning", {
                  scanned: mentions.scanned,
                  total: mentions.total,
                })}
              />
            )}
            {orphanStale.map((group) => renderStaleOnly(group))}
            {visible.map((group) =>
              renderGroup(group, true, links.staleSources.has(group.path)),
            )}
            {mentions.phase === "ready" &&
              mentionGroups.length === 0 &&
              orphanStale.length === 0 && (
                <p className="ts-links__empty">{t("links.mentions.none")}</p>
              )}
            {hidden > 0 && (
              <div className="ts-links__more">
                <button
                  type="button"
                  className="ts-btn"
                  onClick={() => setMentionsShown((shown) => shown + LINKS_PAGE)}
                >
                  {t("links.more", { count: Math.min(LINKS_PAGE, hidden) })}
                </button>
              </div>
            )}
            {readOnly && mentionGroups.length > 0 && (
              <p className="ts-links__note ts-links__note--indent">
                {t("links.mentions.readOnly")}
              </p>
            )}
          </>
        );
      }
    }
  })();

  /** Группа, чьих упоминаний после обновления не осталось: только тег и причина. */
  function renderStaleOnly(group: SourceGroup): React.JSX.Element {
    return (
      <div key={group.path} className="ts-links__group">
        <div className="ts-links__grouphead">
          <span className="ts-links__name">{group.title}</span>
          <span className="ts-spacer" />
          <span className="ts-notepanel__tag ts-notepanel__tag--warn">
            {t("links.mentions.stale")}
          </span>
        </div>
        <p className="ts-links__note ts-links__note--warn ts-links__note--indent">
          {t("links.mentions.staleNote")}
        </p>
      </div>
    );
  }

  return (
    <div className="ts-links">
      <section>
        <div className="ts-links__sec">
          <button
            type="button"
            className="ts-links__sechead ts-eyebrow"
            aria-expanded={backlinksOpen}
            onClick={() => setBacklinksOpen((open) => !open)}
          >
            <span aria-hidden="true">{backlinksOpen ? "▾" : "▸"}</span>{" "}
            {t("links.backlinks.title")}
          </button>
          <span className="ts-spacer" />
          {count !== null && (
            <span
              className="ts-notepanel__tag"
              aria-label={
                count.unit === "links"
                  ? t("links.count", { count: count.value })
                  : t("links.notesCount", { count: count.value })
              }
            >
              {formatNumber(count.value)}
            </span>
          )}
        </div>
        {backlinksOpen && backlinksBody}
      </section>

      <section className="ts-links__section--divided">
        <div className="ts-links__sec">
          <button
            type="button"
            className="ts-links__sechead ts-eyebrow"
            aria-expanded={mentionsExpanded}
            onClick={() => onMentionsExpanded(!mentionsExpanded)}
          >
            <span aria-hidden="true">{mentionsExpanded ? "▾" : "▸"}</span>{" "}
            {t("links.mentions.title")}
          </button>
          <span className="ts-spacer" />
          {!mentionsExpanded && <span className="ts-links__hint">{t("links.mentions.collapsed")}</span>}
          {mentions.phase === "scanning" && (
            <button
              type="button"
              className="ts-btn ts-links__link"
              onClick={() => onMentionsExpanded(false)}
            >
              {t("links.mentions.collapse")}
            </button>
          )}
          {links.mentionCount !== null && (
            <span className="ts-notepanel__tag">{formatNumber(links.mentionCount)}</span>
          )}
        </div>
        {mentionsExpanded && mentionsBody}
      </section>
    </div>
  );
}

/** Фрагмент строки: одно вхождение подсвечено ядром. */
export function Snippet({ segments }: { segments: readonly LineSegment[] }): React.JSX.Element {
  return (
    <>
      {segments.map((part, index) =>
        part.highlighted ? (
          <mark key={index} className="ts-links__mark">
            {part.text}
          </mark>
        ) : (
          <span key={index}>{part.text}</span>
        ),
      )}
    </>
  );
}

/** Индикатор занятости — тот же hazard, что у компиляции, и причина словами. */
function Busy({ text }: { text: string }): React.JSX.Element {
  return (
    <div className="ts-links__busy" role="status">
      <div className="ts-hazard ts-links__hazard">
        <div className="ts-hazard__fill" />
      </div>
      <p className="ts-links__note ts-links__note--flush">{text}</p>
    </div>
  );
}
