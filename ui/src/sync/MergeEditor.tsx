"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import type {
  Annotated,
  Conflict,
  Content,
  LineOrigin,
  Pick,
  Resolution,
  Segment,
} from "../wasm-core/typsidian_core.js";
import { useI18n, type I18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { describeSyncError } from "./describe.js";
import { MergeResult } from "./MergeResult.js";
import { originClass, originMark } from "./mergeMarks.js";
import type { SyncControl } from "./useSync.js";

export interface MergeEditorProps {
  sync: SyncControl;
  conflict: Conflict;
  onClose: () => void;
}

/** Пауза после последнего нажатия, после которой ядро перекрашивает правку. */
const ANNOTATE_DELAY_MS = 250;

const SIDES: readonly Pick[] = ["Ours", "Theirs", "Both"];

type Column = "ours" | "base" | "theirs";

/**
 * Редактор слияния (011 §FR-016, §FR-025, §FR-034; фича 040): своя версия, их
 * версия и — когда история её знает — общая; ниже спорные участки и результат.
 *
 * Всё, что касается слияния, считает ядро: пометки строк колонок и участки с
 * происхождением приходят в `conflict.view`, результат с пометками строк — из
 * `mergeAnnotate`. Здесь только выбор сторон, номер участка и отрисовка.
 */
export function MergeEditor({ sync, conflict, onClose }: MergeEditorProps): React.JSX.Element {
  const i18n = useI18n();
  const { t } = i18n;
  const busy = sync.activity !== null;
  useDismissOnEscape(busy ? null : onClose);

  const view = conflict.view ?? null;
  const segments: Segment[] | null = view?.segments ?? null;
  const disputed = useMemo(
    () => (segments ?? []).filter((segment) => segment.kind === "Disputed").length,
    [segments],
  );

  const [picks, setPicks] = useState<Pick[]>(() => Array<Pick>(disputed).fill("Ours"));
  // Последний массовый выбор: по нему видно, какие участки выбраны иначе.
  const [bulk, setBulk] = useState<Pick | null>(null);
  // Ручная правка результата. `null` — результат собирается из выбора сторон.
  const [edited, setEdited] = useState<string | null>(null);
  const [annotated, setAnnotated] = useState<Annotated | null>(null);
  // Массовый выбор, ждущий подтверждения замены ручной правки (FR-008).
  const [pendingBulk, setPendingBulk] = useState<Pick | null>(null);
  const [current, setCurrent] = useState(0);
  const [tab, setTab] = useState<Column>("ours");
  const hunkRefs = useRef<(HTMLLIElement | null)[]>([]);

  // Ответ ядра на устаревший запрос не должен перекрыть свежий.
  const requestRef = useRef(0);
  useEffect(() => {
    if (segments === null) return undefined;
    const request = ++requestRef.current;
    const ask = (): void => {
      void sync.client
        .mergeAnnotate({ segments, picks, ...(edited === null ? {} : { edited }) })
        .then((next) => {
          if (request === requestRef.current) setAnnotated(next);
        })
        .catch(() => {});
    };
    if (edited === null) {
      ask();
      return undefined;
    }
    const timer = window.setTimeout(ask, ANNOTATE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [sync.client, segments, picks, edited]);

  const result: string | null = segments === null ? null : (edited ?? annotated?.text ?? null);
  const manualLines = useMemo(
    () => (edited === null ? 0 : (annotated?.lines ?? []).filter((l) => l === "Manual").length),
    [edited, annotated],
  );
  const overridden = bulk === null ? 0 : picks.filter((pick) => pick !== bulk).length;

  const applyBulk = (side: Pick): void => {
    setEdited(null);
    setPendingBulk(null);
    setBulk(side);
    setPicks(Array<Pick>(disputed).fill(side));
  };
  const requestBulk = (side: Pick): void => {
    if (edited !== null && manualLines > 0) setPendingBulk(side);
    else applyBulk(side);
  };
  const choose = (index: number, side: Pick): void => {
    setEdited(null);
    setPicks((old) => old.map((pick, at) => (at === index ? side : pick)));
  };
  const goTo = (index: number): void => {
    const next = Math.max(0, Math.min(disputed - 1, index));
    setCurrent(next);
    hunkRefs.current[next]?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  };

  const isSyncthing = conflict.source.kind === "Syncthing";
  const [copyPath, setCopyPath] = useState(() =>
    withSuffix(conflict.path, t("sync.merge.copySuffix")),
  );

  const resolve = async (resolution: Resolution): Promise<void> => {
    const done = await sync.run("resolve", async () => {
      await sync.client.resolve(conflict.id, resolution);
      return true;
    });
    if (done === true) onClose();
  };

  const abortPull = async (): Promise<void> => {
    const done = await sync.run("resolve", async () => {
      await sync.client.abortPull();
      return true;
    });
    if (done === true) onClose();
  };

  const sideLabel = (side: Pick, all: boolean): string => {
    switch (side) {
      case "Ours":
        return all ? t("sync.merge.allOurs") : t("sync.merge.takeOurs");
      case "Theirs":
        return all ? t("sync.merge.allTheirs") : t("sync.merge.takeTheirs");
      case "Both":
        return all ? t("sync.merge.allBoth") : t("sync.merge.takeBoth");
      default: {
        const exhaustive: never = side;
        return exhaustive;
      }
    }
  };

  const columns: { key: Column; title: string; content: Content; marks: LineOrigin[] | null }[] =
    [
      { key: "ours", title: t("sync.merge.ours"), content: conflict.ours, marks: view?.ours ?? null },
      ...(conflict.base === undefined || conflict.base === null
        ? []
        : [
            {
              key: "base" as const,
              title: t("sync.merge.base"),
              content: conflict.base,
              marks: view?.base ?? null,
            },
          ]),
      {
        key: "theirs",
        title: t("sync.merge.theirs"),
        content: conflict.theirs,
        marks: view?.theirs ?? null,
      },
    ];
  const activeTab = columns.some((column) => column.key === tab) ? tab : "ours";

  let disputedIndex = 0;

  return (
    <div
      className="ts-dialog ts-sync-merge"
      role="dialog"
      aria-modal="true"
      aria-label={t("sync.merge.title", { path: conflict.path })}
    >
      <div className="ts-dialog__head">
        <span className="ts-eyebrow">{t("sync.merge.title", { path: conflict.path })}</span>
        <p className="ts-dialog__hint">{sourceText(i18n, conflict)}</p>
      </div>

      <div className="ts-dialog__body">
        {view !== null && (
          <ul className="ts-merge-legend" aria-label={t("sync.merge.legend")}>
            {(["Ours", "Theirs", "Both", "Manual", "Same"] as const).map((origin) => (
              <li key={origin}>
                <span className={`ts-merge-chip ${originClass(origin)}`} aria-hidden="true">
                  {originMark(origin)}
                </span>
                {legendText(i18n, origin)}
              </li>
            ))}
          </ul>
        )}

        <div className="ts-merge-tabs" role="tablist">
          {columns.map((column) => (
            <button
              key={column.key}
              type="button"
              role="tab"
              aria-selected={activeTab === column.key}
              className="ts-merge-tabs__item"
              onClick={() => setTab(column.key)}
            >
              {column.title}
            </button>
          ))}
        </div>

        <div
          className="ts-sync-merge__columns"
          data-columns={columns.length}
          data-tab={activeTab}
        >
          {columns.map((column) => (
            <VersionColumn
              key={column.key}
              column={column.key}
              i18n={i18n}
              title={column.title}
              content={column.content}
              marks={column.marks}
            />
          ))}
        </div>

        {segments !== null && disputed === 0 && (
          <p className="ts-dialog__hint">{t("sync.merge.autoClean")}</p>
        )}

        {segments !== null && disputed > 0 && (
          <section aria-label={t("sync.merge.disputed")} className="ts-merge-disputed">
            <div className="ts-merge-bulk" role="toolbar" aria-label={t("sync.merge.disputed")}>
              <span className="ts-merge-bulk__title">
                {t("sync.merge.disputedCount", { count: disputed })}
              </span>
              <div className="ts-seg" role="group" aria-label={t("sync.merge.allGroup")}>
                {SIDES.map((side) => (
                  <button
                    key={side}
                    type="button"
                    className="ts-seg__item"
                    disabled={busy}
                    aria-pressed={
                      edited === null && bulk === side && overridden === 0 ? true : false
                    }
                    onClick={() => requestBulk(side)}
                  >
                    {sideLabel(side, true)}
                  </button>
                ))}
              </div>
              {overridden > 0 && (
                <span className="ts-merge-bulk__note">
                  {t("sync.merge.overriddenCount", { count: overridden })}
                </span>
              )}
              <span className="ts-spacer" />
              {disputed > 1 && (
                <span className="ts-merge-nav">
                  <button
                    type="button"
                    className="ts-btn ts-btn--ghost ts-btn--sm"
                    aria-label={t("sync.merge.previous")}
                    title={t("sync.merge.previous")}
                    disabled={current === 0}
                    onClick={() => goTo(current - 1)}
                  >
                    ‹
                  </button>
                  {t("sync.merge.position", { index: current + 1, total: disputed })}
                  <button
                    type="button"
                    className="ts-btn ts-btn--ghost ts-btn--sm"
                    aria-label={t("sync.merge.next")}
                    title={t("sync.merge.next")}
                    disabled={current === disputed - 1}
                    onClick={() => goTo(current + 1)}
                  >
                    ›
                  </button>
                </span>
              )}
            </div>

            <ol className="ts-sync-merge__hunks">
              {segments.map((segment, position) => {
                if (segment.kind !== "Disputed") return null;
                const index = disputedIndex++;
                const chosen = picks[index] ?? "Ours";
                const differs = bulk !== null && chosen !== bulk;
                return (
                  <li
                    key={position}
                    ref={(node) => {
                      hunkRefs.current[index] = node;
                    }}
                    className="ts-merge-hunk"
                    aria-current={index === current ? "true" : undefined}
                    onFocus={() => setCurrent(index)}
                  >
                    <div className="ts-merge-hunk__head">
                      <span className="ts-eyebrow">
                        {t("sync.merge.fragment", { index: index + 1 })}
                      </span>
                      <span className="ts-spacer" />
                      {differs && (
                        <span className="ts-merge-hunk__override">
                          {t("sync.merge.overridden")}
                        </span>
                      )}
                      <div className="ts-seg" role="group" aria-label={t("sync.merge.choose")}>
                        {SIDES.map((side) => (
                          <button
                            key={side}
                            type="button"
                            className="ts-seg__item"
                            disabled={busy}
                            aria-pressed={edited === null && chosen === side}
                            onClick={() => choose(index, side)}
                          >
                            {side === "Ours" && `${originMark("Ours")} `}
                            {side === "Both" && `${originMark("Both")} `}
                            {sideLabel(side, false)}
                            {side === "Theirs" && ` ${originMark("Theirs")}`}
                          </button>
                        ))}
                      </div>
                    </div>
                    <div className="ts-sync-merge__pair">
                      <HunkHalf
                        i18n={i18n}
                        text={segment.ours}
                        origin="Ours"
                        picked={chosen === "Ours" || chosen === "Both"}
                      />
                      <HunkHalf
                        i18n={i18n}
                        text={segment.theirs}
                        origin="Theirs"
                        picked={chosen === "Theirs" || chosen === "Both"}
                      />
                    </div>
                  </li>
                );
              })}
            </ol>
          </section>
        )}

        {pendingBulk !== null && (
          <div className="ts-merge-confirm" role="alertdialog" aria-label={t("sync.merge.replaceTitle")}>
            <p>
              <strong>{t("sync.merge.replaceTitle")}</strong>{" "}
              {t("sync.merge.replaceText", { count: manualLines })}
            </p>
            <button type="button" className="ts-btn" onClick={() => setPendingBulk(null)}>
              {t("sync.merge.keepEdit")}
            </button>
            <button
              type="button"
              className="ts-btn ts-btn--accent"
              onClick={() => applyBulk(pendingBulk)}
            >
              {t("sync.merge.replace")}
            </button>
          </div>
        )}

        {segments !== null ? (
          <div className="ts-field">
            <div className="ts-merge-result__head">
              <span className="ts-field__label">{t("sync.merge.result")}</span>
              <span className="ts-field__hint">
                {manualLines > 0
                  ? t("sync.merge.manualCount", { count: manualLines })
                  : t("sync.merge.resultHint")}
              </span>
            </div>
            {result !== null && (
              <MergeResult
                value={result}
                lines={annotated?.lines ?? []}
                ariaLabel={t("sync.merge.result")}
                readOnly={busy}
                onChange={setEdited}
              />
            )}
          </div>
        ) : (
          // Совмещать нечего: двоичный файл, одной из версий нет или нет общей
          // версии — только выбор версии целиком (011 research §11).
          <p className="ts-dialog__hint">{t("sync.merge.wholeOnly")}</p>
        )}

        {isSyncthing && (
          <label className="ts-field">
            <span className="ts-field__label">{t("sync.merge.keepBothPath")}</span>
            <input
              className="ts-field__input ts-field__input--code"
              value={copyPath}
              disabled={busy}
              spellCheck={false}
              onChange={(event) => setCopyPath(event.target.value)}
            />
          </label>
        )}

        {sync.error !== null && (
          <p className="ts-dialog__error" role="alert">
            {describeSyncError(t, sync.error)}
          </p>
        )}
      </div>

      <div className="ts-dialog__foot">
        <button type="button" className="ts-btn" disabled={busy} onClick={onClose}>
          {t("common.close")}
        </button>
        {conflict.source.kind === "Pull" && (
          <button
            type="button"
            className="ts-btn ts-btn--danger"
            disabled={busy}
            title={t("sync.merge.abortPullHint")}
            onClick={() => void abortPull()}
          >
            {t("sync.merge.abortPull")}
          </button>
        )}
        <span className="ts-spacer" />
        {isSyncthing && (
          <button
            type="button"
            className="ts-btn"
            disabled={busy || copyPath.trim() === ""}
            onClick={() => void resolve({ kind: "KeepBothAsFile", new_path: copyPath.trim() })}
          >
            {t("sync.merge.keepBoth")}
          </button>
        )}
        <button
          type="button"
          className="ts-btn"
          disabled={busy}
          onClick={() => void resolve({ kind: "KeepOurs" })}
        >
          {t("sync.merge.keepOurs")}
        </button>
        <button
          type="button"
          className="ts-btn"
          disabled={busy}
          onClick={() => void resolve({ kind: "TakeTheirs" })}
        >
          {t("sync.merge.useTheirs")}
        </button>
        {result !== null && (
          <button
            type="button"
            className="ts-btn ts-btn--accent"
            disabled={busy}
            onClick={() => void resolve({ kind: "Merged", text: result })}
          >
            {t("sync.merge.saveMerged")}
          </button>
        )}
      </div>
    </div>
  );
}

/** Строки текста для показа: последняя пустая «строка» после `\n` не рисуется. */
function splitLines(text: string): string[] {
  if (text === "") return [];
  const lines = text.split("\n");
  if (lines[lines.length - 1] === "") lines.pop();
  return lines;
}

function CodeLines({
  text,
  marks,
  numbered,
}: {
  text: string;
  marks: (index: number) => LineOrigin;
  numbered: boolean;
}): React.JSX.Element {
  return (
    <div className="ts-merge-code">
      {splitLines(text).map((line, index) => {
        const origin = marks(index);
        return (
          <div key={index} className={`ts-merge-line ${originClass(origin)}`}>
            {numbered && <span className="ts-merge-line__no">{index + 1}</span>}
            <span className="ts-merge-line__mark" aria-hidden="true">
              {originMark(origin)}
            </span>
            <span className="ts-merge-line__text">{line}</span>
          </div>
        );
      })}
    </div>
  );
}

function VersionColumn({
  column,
  i18n,
  title,
  content,
  marks,
}: {
  column: Column;
  i18n: I18n;
  title: string;
  content: Content;
  marks: LineOrigin[] | null;
}): React.JSX.Element {
  const { t } = i18n;
  const changed = (marks ?? []).filter((mark) => mark !== "Same").length;
  return (
    <section className="ts-sync-merge__column" aria-label={title} data-column={column}>
      <div className="ts-merge-column__head">
        <h3 className="ts-eyebrow">{title}</h3>
        {marks !== null && changed > 0 && (
          <span className="ts-merge-column__count">
            {t("sync.merge.changedLines", { count: changed })}
          </span>
        )}
      </div>
      {content.kind === "Text" && (
        <CodeLines text={content.text} marks={(index) => marks?.[index] ?? "Same"} numbered />
      )}
      {content.kind === "Binary" && (
        <p className="ts-dialog__hint">
          {t("sync.content.binary", { size: i18n.formatBytes(content.size) })}
        </p>
      )}
      {content.kind === "Missing" && <p className="ts-dialog__hint">{t("sync.content.missing")}</p>}
    </section>
  );
}

function HunkHalf({
  i18n,
  text,
  origin,
  picked,
}: {
  i18n: I18n;
  text: string;
  origin: LineOrigin;
  picked: boolean;
}): React.JSX.Element {
  const { t } = i18n;
  return (
    <div className="ts-merge-half" data-picked={picked ? "true" : "false"}>
      {picked && <span className="ts-merge-half__tag">{t("sync.merge.inResult")}</span>}
      {text === "" ? (
        <p className="ts-merge-half__empty">{t("sync.merge.emptySide")}</p>
      ) : (
        <CodeLines text={text} marks={() => origin} numbered={false} />
      )}
    </div>
  );
}

function legendText(i18n: I18n, origin: LineOrigin): string {
  const { t } = i18n;
  switch (origin) {
    case "Ours":
      return t("sync.merge.legendOurs");
    case "Theirs":
      return t("sync.merge.legendTheirs");
    case "Both":
      return t("sync.merge.legendBoth");
    case "Manual":
      return t("sync.merge.legendManual");
    case "Same":
      return t("sync.merge.legendSame");
    default: {
      const exhaustive: never = origin;
      return exhaustive;
    }
  }
}

/** Откуда конфликт — одной строкой под заголовком. */
function sourceText(i18n: I18n, conflict: Conflict): string {
  const { t } = i18n;
  const source = conflict.source;
  switch (source.kind) {
    case "Pull":
      return t("sync.merge.sourcePull", { snapshot: source.theirs_snapshot.slice(0, 7) });
    case "Syncthing":
      return t("sync.merge.sourceSyncthing", { device: source.device, stamp: source.stamp });
    default: {
      const exhaustive: never = source;
      return exhaustive;
    }
  }
}

/** `папка/имя.typ` → `папка/имя<суффикс>.typ` — имя по умолчанию для второй версии. */
function withSuffix(path: string, suffix: string): string {
  const slash = path.lastIndexOf("/");
  const dot = path.lastIndexOf(".");
  return dot > slash + 1 ? `${path.slice(0, dot)}${suffix}${path.slice(dot)}` : `${path}${suffix}`;
}
