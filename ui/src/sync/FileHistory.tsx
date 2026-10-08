"use client";

import { useCallback, useEffect, useState } from "react";
import type {
  FileDiff,
  Snapshot,
  SyncError,
} from "../wasm-core/typsidian_core.js";
import { DiffView } from "../editor/DiffView.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { describeChange, describeSyncError } from "./describe.js";
import { asSyncError } from "./sync-client.js";
import type { SyncControl } from "./useSync.js";

/** Сколько снимков запрашивается за раз (история читается постранично). */
const PAGE = 30;

export interface FileHistoryProps {
  sync: SyncControl;
  path: string;
  /**
   * Заменить текст открытой заметки правкой редактора — чтобы возврат версии
   * отменялся обычным `Ctrl+Z` (FR-011). Не передан — файл не открыт в
   * редакторе, и версия возвращается записью файла.
   */
  applyToEditor?: ((text: string) => void) | undefined;
  onClose: () => void;
}

/** С чем сравнивать выбранный снимок. */
type Against = "current" | "previous";

/**
 * История файла (FR-009…FR-011): снимки, в которых он менялся, сравнение
 * версий и возврат к выбранной.
 *
 * Список, прослеживание переименований и сам diff приходят из ядра; здесь —
 * выбор снимка и показ.
 */
export function FileHistory({
  sync,
  path,
  applyToEditor,
  onClose,
}: FileHistoryProps): React.JSX.Element {
  const i18n = useI18n();
  const { t } = i18n;
  const { client } = sync;
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [failure, setFailure] = useState<SyncError | null>(null);
  const [selected, setSelected] = useState<Snapshot | null>(null);
  const [against, setAgainst] = useState<Against>("current");
  const [diff, setDiff] = useState<FileDiff | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [restored, setRestored] = useState(false);

  useDismissOnEscape(onClose);

  const load = useCallback(
    async (from: string | null) => {
      setLoading(true);
      try {
        const page = await client.history(path, from, PAGE);
        setSnapshots((known) => (from === null ? page.snapshots : [...known, ...page.snapshots]));
        setCursor(page.cursor ?? null);
        setFailure(null);
      } catch (cause) {
        setFailure(asSyncError(cause));
      } finally {
        setLoading(false);
      }
    },
    [client, path],
  );

  useEffect(() => {
    void load(null);
  }, [load]);

  // Имя файла в выбранном снимке: до переименования оно было другим.
  const nameThen = selected?.path ?? path;
  const renamedSince = nameThen !== path;
  // Сравнить с текущим файлом можно, только пока имя то же: diff ядра идёт
  // по одному пути.
  const effectiveAgainst: Against = renamedSince ? "previous" : against;

  useEffect(() => {
    if (selected === null) {
      setDiff(null);
      return;
    }
    let cancelled = false;
    setDiff(null);
    void (async () => {
      try {
        const next =
          effectiveAgainst === "current"
            ? await client.diff(nameThen, { kind: "Snapshot", id: selected.id }, { kind: "Working" })
            : await client.diff(
                nameThen,
                { kind: "ParentOf", id: selected.id },
                { kind: "Snapshot", id: selected.id },
              );
        if (!cancelled) setDiff(next);
      } catch (cause) {
        if (!cancelled) setFailure(asSyncError(cause));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, selected, effectiveAgainst, nameThen]);

  const restore = async (): Promise<void> => {
    if (selected === null) return;
    setRestoring(true);
    setFailure(null);
    try {
      if (applyToEditor !== undefined) {
        const version = await client.fileAt(nameThen, { kind: "Snapshot", id: selected.id });
        if (version.kind !== "Text") throw { kind: "Io", message: t("sync.history.notText") };
        applyToEditor(version.text);
      } else {
        await client.restore(path, selected.id);
      }
      setRestored(true);
      await sync.refresh();
    } catch (cause) {
      setFailure(asSyncError(cause));
    } finally {
      setRestoring(false);
    }
  };

  // Запись файла ядром идёт по одному пути; версию под прежним именем можно
  // вернуть только правкой открытой заметки.
  const canRestore =
    selected !== null &&
    selected.change?.kind !== "Deleted" &&
    (applyToEditor !== undefined || !renamedSince);

  return (
    <div
      className="ts-dialog ts-sync-history"
      role="dialog"
      aria-modal="true"
      aria-label={t("sync.history.fileTitle", { path })}
    >
      <div className="ts-dialog__head">
        <span className="ts-eyebrow">{t("sync.history.fileTitle", { path })}</span>
      </div>

      <div className="ts-dialog__body ts-sync-history__body">
        <ol className="ts-sync-list" aria-label={t("sync.history.snapshots")}>
          {snapshots.map((snapshot) => (
            <li key={snapshot.id}>
              <button
                type="button"
                className="ts-sync-list__row"
                aria-current={selected?.id === snapshot.id ? "true" : undefined}
                onClick={() => {
                  setSelected(snapshot);
                  setRestored(false);
                }}
              >
                <span className="ts-sync-list__title">{snapshot.summary}</span>
                <span className="ts-sync-list__meta">
                  {i18n.formatDate(new Date(snapshot.time))} · {snapshot.author.name}
                  {snapshot.change !== undefined &&
                    ` · ${describeChange(t, snapshot.change)}`}
                </span>
              </button>
            </li>
          ))}
          {!loading && snapshots.length === 0 && failure === null && (
            <li className="ts-dialog__hint">{t("sync.history.empty")}</li>
          )}
          {loading && <li role="status">{t("common.loading")}</li>}
          {!loading && cursor !== null && (
            <li>
              <button type="button" className="ts-btn ts-btn--quiet" onClick={() => void load(cursor)}>
                {t("sync.history.more")}
              </button>
            </li>
          )}
        </ol>

        <section className="ts-sync-history__diff" aria-label={t("sync.history.comparison")}>
          {selected === null ? (
            <p className="ts-dialog__hint">{t("sync.history.pick")}</p>
          ) : (
            <>
              <div className="ts-seg" role="group" aria-label={t("sync.history.compareWith")}>
                <button
                  type="button"
                  className="ts-seg__item"
                  aria-pressed={effectiveAgainst === "current"}
                  disabled={renamedSince}
                  title={renamedSince ? t("sync.history.renamedHint", { name: nameThen }) : undefined}
                  onClick={() => setAgainst("current")}
                >
                  {t("sync.history.againstCurrent")}
                </button>
                <button
                  type="button"
                  className="ts-seg__item"
                  aria-pressed={effectiveAgainst === "previous"}
                  onClick={() => setAgainst("previous")}
                >
                  {t("sync.history.againstPrevious")}
                </button>
              </div>
              {renamedSince && (
                <p className="ts-dialog__hint">{t("sync.history.renamedHint", { name: nameThen })}</p>
              )}
              {diff === null && failure === null && <p role="status">{t("common.loading")}</p>}
              {diff?.kind === "Text" && (
                <div className="ts-sync-diff">
                  <DiffView diff={diff.lines} />
                </div>
              )}
              {diff?.kind === "Binary" && (
                <p className="ts-dialog__hint">
                  {t("sync.history.binary", {
                    before:
                      diff.old_size === undefined
                        ? t("sync.content.missing")
                        : i18n.formatBytes(diff.old_size),
                    after:
                      diff.new_size === undefined
                        ? t("sync.content.missing")
                        : i18n.formatBytes(diff.new_size),
                  })}
                </p>
              )}
            </>
          )}
          {failure !== null && (
            <p className="ts-dialog__error" role="alert">
              {describeSyncError(t, failure)}
            </p>
          )}
          {restored && (
            <p className="ts-dialog__hint" role="status">
              {applyToEditor !== undefined
                ? t("sync.history.restoredEditor")
                : t("sync.history.restoredFile")}
            </p>
          )}
        </section>
      </div>

      <div className="ts-dialog__foot">
        <button type="button" className="ts-btn" onClick={onClose}>
          {t("common.close")}
        </button>
        <span className="ts-spacer" />
        <button
          type="button"
          className="ts-btn ts-btn--accent"
          disabled={!canRestore || restoring}
          title={
            selected !== null && !canRestore ? t("sync.history.cannotRestore") : undefined
          }
          onClick={() => void restore()}
        >
          {t("sync.history.restore")}
        </button>
      </div>
    </div>
  );
}
