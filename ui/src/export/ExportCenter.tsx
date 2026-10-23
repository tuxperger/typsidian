"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import type {
  BatchJob,
  DiagnosticLocation,
  ExportCandidate,
  ExportFormat,
  VaultOverrides,
} from "../wasm-core/typsidian_core.js";
import type { PreviewClient } from "../preview-client.js";
import { Icon } from "../icons/Icons.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { NOTHING_OVERRIDDEN } from "../workspace/useShortcuts.js";
import type { ExportControl } from "./useExport.js";
import { useI18n, type Translate } from "../i18n/index.js";
import { firstErrorWithPlace, jobLabel } from "./describe.js";

// Центр экспорта (фича 009; макет «Typsidian Утилиты» § Экспорт).
//
// Что можно экспортировать, в каком порядке идёт пакет и что с каждой
// строкой — решает ядро. Здесь выбор строк, раскладка и кнопки.
//
// От макета сознательно отличается: формат только PDF (HTML вне скоупа),
// вывод только отдельными файлами, нет сквозной нумерации, титульного листа и
// паузы — мёртвая кнопка хуже её отсутствия.

export interface ExportCenterProps {
  client: PreviewClient;
  overrides: VaultOverrides | null;
  exporter: ExportControl;
  /** Открытая заметка — при открытии Центра она уже отмечена (FR-014). */
  preselect: string | null;
  onClose: () => void;
  onOpenSettings: () => void;
  onReveal: (place: DiagnosticLocation) => void;
}

export function ExportCenter({
  client,
  overrides,
  exporter,
  preselect,
  onClose,
  onOpenSettings,
  onReveal,
}: ExportCenterProps): React.JSX.Element {
  useDismissOnEscape(onClose);
  const i18n = useI18n();
  const { t } = i18n;

  const settings = overrides ?? NOTHING_OVERRIDDEN;
  const [candidates, setCandidates] = useState<ExportCandidate[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(() => new Set());
  const [stopOnError, setStopOnError] = useState(false);
  // Формат пакета (фича 034): PDF или страницы HTML.
  const [format, setFormat] = useState<ExportFormat>("Pdf");
  const [archiveError, setArchiveError] = useState(false);
  const anchor = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    client.exportCandidates(settings).then(
      (rows) => {
        if (cancelled) return;
        setCandidates(rows);
        // FR-014: открытая заметка уже отмечена — но только при первом
        // показе, чтобы перечитанный список не возвращал снятую отметку.
        setSelected((current) => {
          if (current.size > 0 || preselect === null) return current;
          const row = rows.find((candidate) => candidate.path === preselect);
          return row?.exportable ? new Set([preselect]) : current;
        });
      },
      (cause: unknown) => {
        if (!cancelled) setLoadError(String(cause));
      },
    );
    return () => {
      cancelled = true;
    };
  }, [client, settings, preselect]);

  const exportable = useMemo(
    () => (candidates ?? []).filter((row) => row.exportable).map((row) => row.path),
    [candidates],
  );

  const batch = exporter.batch;
  const running = exporter.batchRunning;
  const jobs = useMemo(() => {
    const byPath = new Map<string, BatchJob>();
    for (const job of batch?.state.jobs ?? []) byPath.set(job.note_path, job);
    return byPath;
  }, [batch]);

  const groups = useMemo(() => groupByFolder(candidates ?? []), [candidates]);

  const allSelected = exportable.length > 0 && exportable.every((path) => selected.has(path));

  const toggle = (path: string, range: boolean): void => {
    if (running) return;
    // Якорь читается здесь, а не в функции обновления: React вызывает её
    // позже, когда якорь уже переставлен на эту строку, и диапазон
    // схлопывался бы в одну строку.
    const start = anchor.current;
    setSelected((current) => {
      const next = new Set(current);
      // FR-012: Shift — все заметки между якорем и этой, в порядке списка.
      if (range && start !== null) {
        const from = exportable.indexOf(start);
        const to = exportable.indexOf(path);
        if (from !== -1 && to !== -1) {
          const [low, high] = from < to ? [from, to] : [to, from];
          for (const between of exportable.slice(low, high + 1)) next.add(between);
          return next;
        }
      }
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
    anchor.current = path;
  };

  const toggleAll = (): void => {
    if (running) return;
    setSelected(allSelected ? new Set() : new Set(exportable));
  };

  const start = (): void => {
    // Порядок пакета — порядок списка (FR-015), а не порядок кликов.
    exporter.startBatch(
      exportable.filter((path) => selected.has(path)),
      stopOnError,
      format,
      { contents: t("export.html.contents"), outside: t("export.html.outside") },
    );
  };

  // Куда пойдёт следующий экспорт — текущая настройка; куда писал прошлый
  // пакет — его собственное значение: папку могли сменить после него.
  const folder = settings.export_folder ?? "export";
  const batchFolder = batch?.state.export_folder ?? folder;
  const progress = batch === null || batch.total === 0 ? 0 : batch.processed / batch.total;

  return (
    <div className="ts-overlay" role="presentation">
      <section className="ts-export" aria-label={t("export.center.title")}>
        <div className="ts-export__head">
          <Icon name="download" size={14} />
          <span className="ts-export__title">{t("export.center.title")}</span>
          <span className="ts-export__meta">
            {t("export.center.selected", { selected: selected.size, total: exportable.length })}
          </span>
          <span className="ts-spacer" />
          <button
            type="button"
            className="ts-btn ts-btn--ghost"
            aria-label={t("export.center.close")}
            onClick={onClose}
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        <div className="ts-export__body">
          <div className="ts-export__list-pane">
            <div className="ts-export__toolbar">
              <button
                type="button"
                className="ts-export__check-label"
                disabled={running || exportable.length === 0}
                onClick={toggleAll}
              >
                <Checkbox on={allSelected} />
                {t("export.center.all")}
              </button>
              <span className="ts-export__hint">{t("export.center.rangeHint")}</span>
              <span className="ts-spacer" />
              <span>{t("export.center.queue", { count: selected.size })}</span>
            </div>

            <div className="ts-export__list">
              {loadError !== null && (
                <p className="ts-export__empty" role="alert">
                  {t("export.center.listFailed", { detail: loadError })}
                </p>
              )}
              {candidates === null && loadError === null && (
                <p className="ts-export__empty">{t("export.center.listLoading")}</p>
              )}
              {candidates !== null && candidates.length === 0 && (
                <p className="ts-export__empty">{t("export.center.noNotes")}</p>
              )}
              {groups.map(([groupFolder, rows]) => (
                <div key={groupFolder || "/"}>
                  <div className="ts-export__group">
                    <Icon name="folder" size={12} />
                    {groupFolder || t("export.center.vaultRoot")}
                  </div>
                  {rows.map((row) => (
                    <ExportRow
                      key={row.path}
                      row={row}
                      job={jobs.get(row.path)}
                      checked={selected.has(row.path)}
                      locked={running}
                      onToggle={(range) => toggle(row.path, range)}
                      onSkip={() => exporter.skip(row.path)}
                      onReveal={onReveal}
                    />
                  ))}
                </div>
              ))}
            </div>
          </div>

          <aside className="ts-export__side">
            <div className="ts-export__side-scroll">
              <div className="ts-export__field">
                <span className="ts-export__label">{t("export.center.format")}</span>
                <div className="ts-export__seg" role="group" aria-label={t("export.center.format")}>
                  {(["Pdf", "Html"] as const).map((id) => (
                    <button
                      key={id}
                      type="button"
                      className="ts-export__seg-item"
                      aria-current={format === id ? "true" : undefined}
                      disabled={running}
                      onClick={() => setFormat(id)}
                    >
                      {id === "Pdf" ? "PDF" : "HTML"}
                    </button>
                  ))}
                </div>
                <span className="ts-export__note">
                  {format === "Pdf" ? t("export.center.formatNote") : t("export.html.formatNote")}
                </span>
              </div>

              <div className="ts-export__field">
                <span className="ts-export__label">{t("export.center.output")}</span>
                <div className="ts-export__folder">
                  <Icon name="folder" size={11} />
                  <span className="ts-export__folder-path">{folder}/</span>
                  <button
                    type="button"
                    className="ts-export__folder-edit"
                    onClick={onOpenSettings}
                    title={t("export.center.folderHint")}
                  >
                    {t("export.center.change")}
                  </button>
                </div>
                <span className="ts-export__note">
                  {format === "Pdf" ? t("export.center.outputNote") : t("export.html.outputNote")}
                </span>
              </div>

              <div className="ts-export__field">
                <span className="ts-export__label">{t("export.center.options")}</span>
                <button
                  type="button"
                  className="ts-export__check-label ts-export__check-label--row"
                  disabled={running}
                  aria-pressed={stopOnError}
                  onClick={() => setStopOnError((on) => !on)}
                >
                  <Checkbox on={stopOnError} />
                  {t("export.center.stopOnError")}
                </button>
              </div>

              {batch !== null && (
                <>
                  <div className="ts-export__rule" />
                  <div className="ts-export__progress">
                    <div className="ts-export__progress-row">
                      <span className="ts-export__progress-key">
                        {t("export.center.progress")}
                      </span>
                      <span className="ts-export__progress-value">
                        {batch.processed} / {batch.total}
                      </span>
                    </div>
                    <div
                      className="ts-export__bar"
                      role="progressbar"
                      aria-valuemin={0}
                      aria-valuemax={batch.total}
                      aria-valuenow={batch.processed}
                    >
                      <div
                        className="ts-export__bar-fill"
                        style={{ width: `${Math.round(progress * 100)}%` }}
                      />
                    </div>
                    {running && (
                      <div className="ts-export__progress-row">
                        <span className="ts-export__progress-key">
                          {t("export.center.remaining")}
                        </span>
                        <span>
                          {batch.remaining_estimate_ms === undefined
                            ? t("export.center.estimatePending")
                            : `~${i18n.formatDuration(batch.remaining_estimate_ms)}`}
                        </span>
                      </div>
                    )}
                    {batch.is_over && (
                      <p className="ts-export__summary" role="status">
                        {summaryText(t, batch.state.phase)}{" "}
                        {t("export.summary.done", { count: batch.summary.done })}
                        {batch.summary.failed > 0 &&
                          t("export.summary.failed", { count: batch.summary.failed })}
                        {batch.summary.not_exported > 0 &&
                          t("export.summary.notExported", { count: batch.summary.not_exported })}
                        {t("export.summary.folder", { folder: batchFolder })}
                      </p>
                    )}
                    {batch.is_over && exporter.siteArchive !== null && (
                      <button
                        type="button"
                        className="ts-btn"
                        onClick={() => {
                          const archive = exporter.siteArchive;
                          if (archive === null) return;
                          setArchiveError(false);
                          void archive()
                            .then((bytes) => downloadArchive(bytes, `${batchFolder}.zip`))
                            .catch(() => setArchiveError(true));
                        }}
                      >
                        <Icon name="download" size={12} />
                        {t("export.html.archive")}
                      </button>
                    )}
                    {archiveError && (
                      <p className="ts-export__summary" role="alert">
                        {t("export.html.archiveFailed")}
                      </p>
                    )}
                  </div>
                </>
              )}

              {exporter.recent.length > 0 && (
                <div className="ts-export__field">
                  <span className="ts-export__label">{t("export.center.recent")}</span>
                  <ul className="ts-export__recent">
                    {exporter.recent.map((entry) => (
                      <li key={entry.pdfPath}>
                        <span className="ts-export__recent-path">{entry.pdfPath}</span>
                        <span className="ts-export__recent-meta">{i18n.formatBytes(entry.bytes)}</span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>

            <div className="ts-export__actions">
              {running ? (
                <button type="button" className="ts-btn ts-btn--danger" onClick={exporter.cancel}>
                  {t("export.center.cancel")}
                </button>
              ) : (
                <>
                  <button
                    type="button"
                    className="ts-btn ts-btn--accent"
                    disabled={selected.size === 0}
                    onClick={start}
                  >
                    {t("export.center.start", { count: selected.size })}
                  </button>
                  {batch !== null && (
                    <button
                      type="button"
                      className="ts-btn ts-btn--quiet"
                      onClick={exporter.clearBatch}
                    >
                      {t("export.center.clear")}
                    </button>
                  )}
                </>
              )}
            </div>
          </aside>
        </div>

        <div className="ts-export__foot">
          <span className="ts-export__badge">EXPORT</span>{/* i18n-ignore: марка-плашка, как словесный знак */}
          <span>{t("export.center.footer")}</span>
        </div>
      </section>
    </div>
  );
}

function ExportRow({
  row,
  job,
  checked,
  locked,
  onToggle,
  onSkip,
  onReveal,
}: {
  row: ExportCandidate;
  job: BatchJob | undefined;
  checked: boolean;
  locked: boolean;
  onToggle: (range: boolean) => void;
  onSkip: () => void;
  onReveal: (place: DiagnosticLocation) => void;
}): React.JSX.Element {
  const i18n = useI18n();
  const { t } = i18n;
  const status = job?.status;
  const place =
    status === "Failed" && job?.report !== undefined
      ? firstErrorWithPlace(job.report)?.location
      : undefined;

  let className = "ts-export__row";
  if (!row.exportable) className += " ts-export__row--off";
  if (status === "Exporting") className += " ts-export__row--active";
  if (status === "Done" || status === "DoneWithWarnings") className += " ts-export__row--done";

  return (
    <div className={className}>
      <button
        type="button"
        className="ts-export__row-check"
        disabled={!row.exportable || locked}
        aria-pressed={checked}
        aria-label={
          checked
            ? t("export.row.uncheck", { name: row.name })
            : t("export.row.check", { name: row.name })
        }
        onClick={(event) => onToggle(event.shiftKey)}
      >
        <Checkbox on={checked} />
      </button>
      <Icon name={row.reason === "Board" ? "board" : "file"} size={13} />
      <span className="ts-export__row-name">
        {row.name}
        {status === "Exporting" && (
          <span className="ts-hazard ts-export__row-hazard" aria-hidden="true">
            <span className="ts-hazard__fill" />
          </span>
        )}
      </span>
      <span className={`ts-export__state ts-export__state--${stateTone(status, row)}`}>
        {!row.exportable
          ? row.reason === "InsideExportFolder"
            ? t("export.row.insideExportFolder")
            : t("export.row.notExportable")
          : status === undefined
            ? ""
            : jobLabel(i18n, status, job?.report, job?.elapsed_ms)}
        {(job?.report?.as_images ?? []).length > 0 && (
          <span className="ts-export__images">
            {t("export.html.asImages", {
              count: (job?.report?.as_images ?? []).length,
              names: (job?.report?.as_images ?? []).join(", "),
            })}
          </span>
        )}
        {status === "Queued" && (
          <button type="button" className="ts-export__skip" onClick={onSkip}>
            {t("export.row.skip")}
          </button>
        )}
        {place && (
          <button type="button" className="ts-export__skip" onClick={() => onReveal(place)}>
            {t("export.row.toError")}
          </button>
        )}
      </span>
    </div>
  );
}

function Checkbox({ on }: { on: boolean }): React.JSX.Element {
  return (
    <span className={on ? "ts-export__cb ts-export__cb--on" : "ts-export__cb"} aria-hidden="true">
      {on && <Icon name="check" size={8} />}
    </span>
  );
}

function stateTone(status: BatchJob["status"] | undefined, row: ExportCandidate): string {
  if (!row.exportable) return "off";
  switch (status) {
    case "Done":
      return "done";
    case "DoneWithWarnings":
      return "warn";
    case "Failed":
      return "failed";
    case "Exporting":
      return "active";
    default:
      return "idle";
  }
}

function summaryText(t: Translate, phase: { kind: string; note_path?: string }): string {
  switch (phase.kind) {
    case "Cancelled":
      return t("export.summary.cancelled");
    case "StoppedOnError":
      return phase.note_path === undefined
        ? t("export.summary.stoppedUnknown")
        : t("export.summary.stopped", { note: phase.note_path });
    default:
      return t("export.summary.finished");
  }
}

/** Группы по папке в порядке списка ядра — как в дереве. */
function groupByFolder(rows: ExportCandidate[]): [string, ExportCandidate[]][] {
  const groups = new Map<string, ExportCandidate[]>();
  for (const row of rows) {
    const list = groups.get(row.folder);
    if (list) list.push(row);
    else groups.set(row.folder, [row]);
  }
  return [...groups.entries()];
}

/** Скачивание архива сайта (web, фича 034): файл через ссылку на `Blob`. */
function downloadArchive(bytes: Uint8Array, name: string): void {
  const url = URL.createObjectURL(new Blob([bytes.slice()], { type: "application/zip" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}
