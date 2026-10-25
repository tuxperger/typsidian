"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type {
  Change,
  Conflict,
  FileDiff,
  IgnoreFile,
  IgnoreReport,
  Snapshot,
  SyncError,
  SyncOverview,
} from "../wasm-core/typsidian_core.js";
import { DiffView } from "../editor/DiffView.js";
import { useI18n, type I18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import {
  describeChange,
  describeFolderState,
  describeInProgress,
  describeLink,
  describeSyncError,
} from "./describe.js";
import { EnableHistoryWizard } from "./EnableHistoryWizard.js";
import { IgnoreEditor } from "./IgnoreEditor.js";
import { MergeEditor } from "./MergeEditor.js";
import { RemoteDialog } from "./RemoteDialog.js";
import { asSyncError, remoteCapabilities, type DiscoveredSyncthing } from "./sync-client.js";
import { SyncSettingsSection } from "./SyncSettingsSection.js";
import type { KeeperConfirmation, SyncControl } from "./useSync.js";

/** Сколько снимков истории vault запрашивается за раз. */
const HISTORY_PAGE = 20;

export interface SyncPanelProps {
  sync: SyncControl;
  /** Раздел, к которому панель прокручивается при открытии. */
  focus?: "conflicts" | null;
  onOpenFileHistory: (path: string) => void;
  onClose: () => void;
}

/**
 * Панель «Синхронизация» (contracts/frontend-sync.md): история, удалённый
 * репозиторий, Syncthing, конфликты, хранитель.
 *
 * Всё, что здесь показано, — сводка ядра (`SyncOverview`) и ответы его
 * операций; панель не решает, можно ли действие, а показывает ответ. Чего
 * оболочка не умеет, не прячется: действие отключено и названа причина
 * (FR-036).
 */
export function SyncPanel({
  sync,
  focus = null,
  onOpenFileHistory,
  onClose,
}: SyncPanelProps): React.JSX.Element {
  const i18n = useI18n();
  const { t } = i18n;
  const { overview } = sync;
  const [wizardOpen, setWizardOpen] = useState(false);
  const [remoteOpen, setRemoteOpen] = useState(false);
  const [remoteError, setRemoteError] = useState<SyncError | null>(null);
  const [merging, setMerging] = useState<Conflict | null>(null);
  const [proxy, setProxy] = useState<string | null>(null);
  const [rulesFile, setRulesFile] = useState<IgnoreFile | null>(null);

  const overlayOpen = wizardOpen || remoteOpen || merging !== null || rulesFile !== null;
  // Диалог поверх панели закрывается первым и своим обработчиком.
  useDismissOnEscape(overlayOpen ? null : onClose);

  const conflictsRef = useRef<HTMLElement | null>(null);
  useEffect(() => {
    if (focus === "conflicts") conflictsRef.current?.scrollIntoView({ block: "start" });
  }, [focus]);

  const openRemote = useCallback(() => {
    setRemoteError(null);
    void sync.client
      .deviceSync()
      .then((device) => setProxy(device.cors_proxy ?? null))
      .catch(() => setProxy(null))
      .finally(() => setRemoteOpen(true));
  }, [sync.client]);

  return (
    <aside className="ts-sync" aria-label={t("sync.panel.title")}>
      <div className="ts-sync__head">
        <span className="ts-eyebrow">{t("sync.panel.title")}</span>
        <span className="ts-spacer" />
        <button type="button" className="ts-btn" onClick={onClose}>
          {t("common.close")}
        </button>
      </div>

      <div className="ts-sync__body">
        {sync.error !== null && (
          <ErrorNotice sync={sync} error={sync.error} onReplaceAccess={openRemote} />
        )}

        {overview === null ? (
          <p role="status">{t("common.loading")}</p>
        ) : (
          <>
            {overview.ignore_warnings.map((report) => (
              <IgnoreWarning
                key={report.file}
                sync={sync}
                report={report}
                onOpen={() => setRulesFile(report.file)}
              />
            ))}

            <HistorySection
              sync={sync}
              overview={overview}
              i18n={i18n}
              onEnable={() => setWizardOpen(true)}
              onOpenFileHistory={onOpenFileHistory}
            />
            <RemoteSection sync={sync} overview={overview} i18n={i18n} onConnect={openRemote} />
            <SyncthingSection sync={sync} overview={overview} />
            <ConflictsSection
              sync={sync}
              overview={overview}
              sectionRef={conflictsRef}
              onMerge={setMerging}
            />
            <KeeperSection sync={sync} overview={overview} i18n={i18n} />
            <RulesSection onOpen={setRulesFile} />

            <details className="ts-sync__section">
              <summary className="ts-eyebrow">{t("sync.settings.title")}</summary>
              <SyncSettingsSection sync={sync} />
            </details>
          </>
        )}
      </div>

      {wizardOpen && <EnableHistoryWizard sync={sync} onClose={() => setWizardOpen(false)} />}

      {remoteOpen && overview !== null && (
        <RemoteDialog
          mode="connect"
          capabilities={remoteCapabilities(overview.capabilities)}
          initialProxy={proxy}
          initialUrl={overview.remote?.url ?? null}
          busy={sync.activity === "connect"}
          progress={sync.progress}
          error={remoteError}
          onCancel={() => {
            if (sync.activity === "connect") sync.cancel();
            else setRemoteOpen(false);
          }}
          onSubmit={(request) => {
            setRemoteError(null);
            void (async () => {
              // Отказ подключения показывает сам диалог, поэтому он ловится
              // здесь, а не уходит в общий индикатор.
              const outcome: { failure: SyncError | null } = { failure: null };
              await sync.run("connect", async () => {
                try {
                  if (!overview.capabilities.direct_remote) {
                    await sync.client.setCorsProxy(request.corsProxy);
                  }
                  await sync.client.connectRemote(request.url, request.credentials);
                } catch (cause) {
                  outcome.failure = asSyncError(cause);
                }
              });
              if (outcome.failure === null) setRemoteOpen(false);
              else setRemoteError(outcome.failure);
            })();
          }}
        />
      )}

      {merging !== null && (
        <MergeEditor sync={sync} conflict={merging} onClose={() => setMerging(null)} />
      )}

      {rulesFile !== null && (
        <IgnoreEditor sync={sync} file={rulesFile} onClose={() => setRulesFile(null)} />
      )}
    </aside>
  );
}

// ── Отказ и предупреждения ─────────────────────────────────────────────────

function ErrorNotice({
  sync,
  error,
  onReplaceAccess,
}: {
  sync: SyncControl;
  error: SyncError;
  onReplaceAccess: () => void;
}): React.JSX.Element {
  const { t } = useI18n();
  return (
    <div className="ts-sync__notice" data-tone="error" role="alert">
      <p>{describeSyncError(t, error)}</p>
      <div className="ts-sync__actions">
        {/* Отказ ведёт к действию, которое его снимает (T046, T065). */}
        {(error.kind === "AuthRejected" ||
          error.kind === "AuthRequired" ||
          error.kind === "RemoteChanged") && (
          <button type="button" className="ts-btn" onClick={onReplaceAccess}>
            {t("sync.remote.replaceAccess")}
          </button>
        )}
        {error.kind === "RejectedNonFastForward" && (
          <button type="button" className="ts-btn" onClick={() => void sync.pull()}>
            {t("sync.remote.pullFirst")}
          </button>
        )}
        {error.kind === "CorsBlocked" && (
          <button type="button" className="ts-btn" onClick={onReplaceAccess}>
            {t("sync.remote.setProxy")}
          </button>
        )}
        <button type="button" className="ts-btn ts-btn--quiet" onClick={sync.dismissError}>
          {t("common.ok")}
        </button>
      </div>
    </div>
  );
}

/** Предупреждение о недостающих правилах исключения (FR-026, FR-027). */
function IgnoreWarning({
  sync,
  report,
  onOpen,
}: {
  sync: SyncControl;
  report: IgnoreReport;
  onOpen: () => void;
}): React.JSX.Element {
  const { t } = useI18n();
  const [shown, setShown] = useState(false);
  const busy = sync.activity !== null;
  return (
    <div className="ts-sync__notice" data-tone="attention" role="status">
      <p>
        {report.file === "StIgnore"
          ? t("sync.ignore.stignoreMissing")
          : t("sync.ignore.gitignoreMissing")}
      </p>
      {shown && <pre className="ts-sync-rules">{report.proposed_block}</pre>}
      <div className="ts-sync__actions">
        <button
          type="button"
          className="ts-btn ts-btn--accent"
          disabled={busy}
          onClick={() => void sync.run("resolve", () => sync.client.applyIgnore(report.file))}
        >
          {t("sync.ignore.fix")}
        </button>
        <button type="button" className="ts-btn" onClick={() => setShown((value) => !value)}>
          {shown ? t("sync.ignore.hideRules") : t("sync.ignore.showRules")}
        </button>
        <button type="button" className="ts-btn" onClick={onOpen}>
          {t("sync.ignore.open")}
        </button>
        {report.file === "StIgnore" && (
          <button
            type="button"
            className="ts-btn ts-btn--quiet"
            disabled={busy}
            title={t("sync.wizard.stignoreDeclineWarning")}
            onClick={() => void sync.run("resolve", () => sync.client.declineStignore())}
          >
            {t("sync.wizard.stignoreDecline")}
          </button>
        )}
      </div>
    </div>
  );
}

// ── История ────────────────────────────────────────────────────────────────

function HistorySection({
  sync,
  overview,
  i18n,
  onEnable,
  onOpenFileHistory,
}: {
  sync: SyncControl;
  overview: SyncOverview;
  i18n: I18n;
  onEnable: () => void;
  onOpenFileHistory: (path: string) => void;
}): React.JSX.Element {
  const { t } = i18n;
  const history = overview.history;
  const busy = sync.activity !== null;

  return (
    <section className="ts-sync__section" aria-label={t("sync.history.title")}>
      <h2 className="ts-eyebrow">{t("sync.history.title")}</h2>

      {history.kind === "Absent" && (
        <>
          <p>
            {overview.syncthing.detected
              ? t("sync.history.absentSyncthing")
              : t("sync.history.absent")}
          </p>
          <button type="button" className="ts-btn ts-btn--accent" onClick={onEnable}>
            {t("sync.history.enable")}
          </button>
        </>
      )}

      {history.kind === "Lost" && (
        <>
          <p className="ts-dialog__warn">{t("sync.history.lost")}</p>
          {/* История цела в удалённом репозитории — вернуть её оттуда лучше,
              чем начинать новую: новая с прежней уже не сойдётся (T079). */}
          {overview.settings.remote_url !== undefined && (
            <>
              <p className="ts-dialog__hint">{t("sync.history.restoreFromRemoteHint")}</p>
              <button
                type="button"
                className="ts-btn ts-btn--accent"
                disabled={busy}
                onClick={() =>
                  void sync.runGated("pull", (confirm) => sync.client.restoreHistory(confirm))
                }
              >
                {t("sync.history.restoreFromRemote")}
              </button>
            </>
          )}
          <button
            type="button"
            className={
              overview.settings.remote_url === undefined ? "ts-btn ts-btn--accent" : "ts-btn"
            }
            onClick={onEnable}
          >
            {t("sync.history.createNew")}
          </button>
        </>
      )}

      {history.kind === "ForeignParent" && (
        <p>{t("sync.history.foreign", { root: history.repo_root })}</p>
      )}

      {history.kind === "Mirror" && (
        <>
          <p>
            {t("sync.history.mirror", {
              keeper: overview.keeper?.device_name ?? t("sync.keeper.unknown"),
            })}{" "}
            {history.fetched_at === undefined
              ? t("sync.history.mirrorNever")
              : t("sync.history.mirrorAt", {
                  time: i18n.formatDate(new Date(history.fetched_at)),
                })}
          </p>
          <button
            type="button"
            className="ts-btn"
            disabled={busy}
            onClick={() => void sync.run("pull", () => sync.client.refreshMirror())}
          >
            {t("sync.history.refreshMirror")}
          </button>
          <VaultHistory sync={sync} i18n={i18n} onOpenFileHistory={onOpenFileHistory} />
        </>
      )}

      {history.kind === "Local" && (
        <>
          {history.in_progress !== undefined && (
            <p className="ts-dialog__warn">
              {describeInProgress(t, history.in_progress)} {t("sync.history.inProgressHint")}
            </p>
          )}
          <ChangesList sync={sync} onOpenFileHistory={onOpenFileHistory} />
          <VaultHistory sync={sync} i18n={i18n} onOpenFileHistory={onOpenFileHistory} />
        </>
      )}
    </section>
  );
}

/** Изменения с последнего снимка, выбор и сам снимок (FR-004…FR-006). */
function ChangesList({
  sync,
  onOpenFileHistory,
}: {
  sync: SyncControl;
  onOpenFileHistory: (path: string) => void;
}): React.JSX.Element {
  const { t } = useI18n();
  const { client, revision } = sync;
  const [changes, setChanges] = useState<Change[] | null>(null);
  // Снятые отметки, а не поставленные: новое изменение по умолчанию входит в
  // снимок, и забыть его случайно нельзя.
  const [excluded, setExcluded] = useState<ReadonlySet<string>>(new Set());
  const [message, setMessage] = useState("");
  const [viewing, setViewing] = useState<string | null>(null);
  const [diff, setDiff] = useState<FileDiff | null>(null);
  const busy = sync.activity !== null;

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const next = await client.changes();
        if (!cancelled) setChanges(next);
      } catch {
        // Причину показывает сводка (нет истории, чужой репозиторий) — список
        // просто пуст.
        if (!cancelled) setChanges([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, revision]);

  useEffect(() => {
    if (viewing === null) {
      setDiff(null);
      return;
    }
    let cancelled = false;
    setDiff(null);
    void (async () => {
      try {
        const next = await client.diff(viewing, { kind: "Head" }, { kind: "Working" });
        if (!cancelled) setDiff(next);
      } catch {
        if (!cancelled) setViewing(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, viewing, revision]);

  if (changes === null) return <p role="status">{t("common.loading")}</p>;

  const selected = changes.filter((change) => !excluded.has(change.path));
  const snapshot = async (): Promise<void> => {
    // Все отмечены — `null`: ядро снимает всё, включая то, что изменилось,
    // пока список был на экране.
    const paths = selected.length === changes.length ? null : selected.map((c) => c.path);
    if (await sync.snapshot(paths, message)) {
      setMessage("");
      setExcluded(new Set());
      setViewing(null);
    }
  };

  return (
    <>
      {changes.length === 0 ? (
        <p className="ts-dialog__hint">{t("sync.changes.none")}</p>
      ) : (
        <ul className="ts-sync-list" aria-label={t("sync.changes.label")}>
          {changes.map((change) => (
            <li key={change.path} className="ts-sync-change">
              <input
                type="checkbox"
                checked={!excluded.has(change.path)}
                aria-label={t("sync.changes.include", { path: change.path })}
                onChange={(event) => {
                  const include = event.target.checked;
                  setExcluded((current) => {
                    const next = new Set(current);
                    if (include) next.delete(change.path);
                    else next.add(change.path);
                    return next;
                  });
                }}
              />
              <button
                type="button"
                className="ts-sync-list__row"
                aria-expanded={viewing === change.path}
                title={t("sync.changes.compare")}
                onClick={() => setViewing((path) => (path === change.path ? null : change.path))}
              >
                <span className="ts-sync-list__title">{change.path}</span>
                <span className="ts-sync-list__meta">{describeChange(t, change.change)}</span>
              </button>
              <button
                type="button"
                className="ts-btn ts-btn--quiet"
                onClick={() => onOpenFileHistory(change.path)}
              >
                {t("sync.history.open")}
              </button>
              {viewing === change.path && (
                <div className="ts-sync-diff">
                  {diff === null && <p role="status">{t("common.loading")}</p>}
                  {diff?.kind === "Text" && <DiffView diff={diff.lines} />}
                  {diff?.kind === "Binary" && (
                    <p className="ts-dialog__hint">{t("sync.changes.binary")}</p>
                  )}
                </div>
              )}
            </li>
          ))}
        </ul>
      )}

      <label className="ts-field">
        <span className="ts-field__label">{t("sync.snapshot.message")}</span>
        <input
          className="ts-field__input"
          value={message}
          disabled={busy}
          placeholder={t("sync.snapshot.messagePlaceholder")}
          onChange={(event) => setMessage(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && selected.length > 0 && !busy) {
              event.preventDefault();
              void snapshot();
            }
          }}
        />
      </label>
      <button
        type="button"
        className="ts-btn ts-btn--accent"
        disabled={busy || selected.length === 0}
        onClick={() => void snapshot()}
      >
        {selected.length === changes.length
          ? t("sync.snapshot.all")
          : t("sync.snapshot.selected", { count: selected.length })}
      </button>
    </>
  );
}

/** История vault и восстановление удалённых файлов (FR-009, FR-011). */
function VaultHistory({
  sync,
  i18n,
  onOpenFileHistory,
}: {
  sync: SyncControl;
  i18n: I18n;
  onOpenFileHistory: (path: string) => void;
}): React.JSX.Element {
  const { t } = i18n;
  const { client, revision } = sync;
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [opened, setOpened] = useState<string | null>(null);
  const [changes, setChanges] = useState<Change[] | null>(null);
  const busy = sync.activity !== null;

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const page = await client.history(null, null, HISTORY_PAGE);
        if (cancelled) return;
        setSnapshots(page.snapshots);
        setCursor(page.cursor ?? null);
      } catch {
        if (!cancelled) setSnapshots([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, revision]);

  useEffect(() => {
    if (opened === null) {
      setChanges(null);
      return;
    }
    let cancelled = false;
    setChanges(null);
    void (async () => {
      try {
        const next = await client.snapshotChanges(opened);
        if (!cancelled) setChanges(next);
      } catch {
        if (!cancelled) setChanges([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, opened]);

  const more = async (): Promise<void> => {
    if (cursor === null) return;
    try {
      const page = await client.history(null, cursor, HISTORY_PAGE);
      setSnapshots((known) => [...known, ...page.snapshots]);
      setCursor(page.cursor ?? null);
    } catch {
      setCursor(null);
    }
  };

  return (
    <>
      <h3 className="ts-eyebrow">{t("sync.history.snapshots")}</h3>
      {snapshots.length === 0 ? (
        <p className="ts-dialog__hint">{t("sync.history.empty")}</p>
      ) : (
        <ol className="ts-sync-list">
          {snapshots.map((snapshot) => (
            <li key={snapshot.id}>
              <button
                type="button"
                className="ts-sync-list__row"
                aria-expanded={opened === snapshot.id}
                onClick={() => setOpened((id) => (id === snapshot.id ? null : snapshot.id))}
              >
                <span className="ts-sync-list__title">{snapshot.summary}</span>
                <span className="ts-sync-list__meta">
                  {i18n.formatDate(new Date(snapshot.time))} · {snapshot.author.name}
                </span>
              </button>
              {opened === snapshot.id && (
                <ul className="ts-sync-list ts-sync-list--nested">
                  {changes === null && <li role="status">{t("common.loading")}</li>}
                  {changes?.map((change) => {
                    const parent = snapshot.parents[0];
                    return (
                      <li key={change.path} className="ts-sync-change">
                        <span className="ts-sync-list__title">{change.path}</span>
                        <span className="ts-sync-list__meta">
                          {describeChange(t, change.change)}
                        </span>
                        {change.change.kind === "Deleted" && parent !== undefined ? (
                          // Удалённый файл возвращается из снимка, где он ещё
                          // был, — на прежнее место (FR-011).
                          <button
                            type="button"
                            className="ts-btn ts-btn--quiet"
                            disabled={busy}
                            onClick={() =>
                              void sync.run("resolve", () =>
                                client.restore(change.path, parent),
                              )
                            }
                          >
                            {t("sync.history.restoreDeleted")}
                          </button>
                        ) : (
                          <button
                            type="button"
                            className="ts-btn ts-btn--quiet"
                            onClick={() => onOpenFileHistory(change.path)}
                          >
                            {t("sync.history.open")}
                          </button>
                        )}
                      </li>
                    );
                  })}
                </ul>
              )}
            </li>
          ))}
          {cursor !== null && (
            <li>
              <button type="button" className="ts-btn ts-btn--quiet" onClick={() => void more()}>
                {t("sync.history.more")}
              </button>
            </li>
          )}
        </ol>
      )}
    </>
  );
}

// ── Удалённый репозиторий ──────────────────────────────────────────────────

function RemoteSection({
  sync,
  overview,
  i18n,
  onConnect,
}: {
  sync: SyncControl;
  overview: SyncOverview;
  i18n: I18n;
  onConnect: () => void;
}): React.JSX.Element {
  const { t } = i18n;
  const remote = overview.remote;
  const busy = sync.activity !== null;
  const hasHistory = overview.history.kind === "Local";
  // Причина, по которой обмен сейчас недоступен, — подсказкой на кнопке
  // (FR-036): действие не прячется.
  const blocked = !hasHistory
    ? t("sync.remote.needsHistory")
    : overview.conflicts > 0 && overview.pull_in_progress
      ? t("sync.error.pullConflictsPending")
      : undefined;

  return (
    <section className="ts-sync__section" aria-label={t("sync.remote.title")}>
      <h2 className="ts-eyebrow">{t("sync.remote.title")}</h2>
      {remote === undefined ? (
        <>
          {/* Оболочка без удалённого (демо-сервер, фича 039): кнопка видна, но
              выключена, и причина названа. */}
          <p className="ts-dialog__hint">
            {overview.capabilities.remote === false
              ? t("sync.remote.unavailable")
              : t("sync.remote.none")}
          </p>
          <button
            type="button"
            className="ts-btn"
            disabled={!hasHistory || busy || overview.capabilities.remote === false}
            title={hasHistory ? undefined : t("sync.remote.needsHistory")}
            onClick={onConnect}
          >
            {t("sync.remote.connectEllipsis")}
          </button>
        </>
      ) : (
        <>
          <p className="ts-sync__url">{remote.url}</p>
          <p className="ts-dialog__hint">
            {remote.ahead === undefined || remote.behind === undefined
              ? t("sync.remote.notCounted")
              : t("sync.remote.counts", { ahead: remote.ahead, behind: remote.behind })}
            {" · "}
            {remote.fetched_at === undefined
              ? t("sync.remote.neverExchanged")
              : t("sync.remote.exchangedAt", {
                  time: i18n.formatDate(new Date(remote.fetched_at)),
                })}
          </p>
          {remote.last_error !== undefined && (
            <p className="ts-dialog__error">{describeSyncError(t, remote.last_error)}</p>
          )}
          {overview.pull_in_progress && (
            <p className="ts-dialog__warn">{t("sync.remote.pullPending")}</p>
          )}
          <div className="ts-sync__actions">
            {sync.activity === "push" || sync.activity === "pull" ? (
              <button type="button" className="ts-btn" onClick={sync.cancel}>
                {t("sync.cancelOperation")}
              </button>
            ) : (
              <>
                <button
                  type="button"
                  className="ts-btn ts-btn--accent"
                  disabled={busy || blocked !== undefined}
                  title={blocked}
                  onClick={() => void sync.push()}
                >
                  {t("sync.remote.push")}
                </button>
                <button
                  type="button"
                  className="ts-btn"
                  disabled={busy || blocked !== undefined}
                  title={blocked}
                  onClick={() => void sync.pull()}
                >
                  {t("sync.remote.pull")}
                </button>
              </>
            )}
            {overview.pull_in_progress && (
              <button
                type="button"
                className="ts-btn ts-btn--danger"
                disabled={busy}
                title={t("sync.merge.abortPullHint")}
                onClick={() => void sync.run("resolve", () => sync.client.abortPull())}
              >
                {t("sync.merge.abortPull")}
              </button>
            )}
            <span className="ts-spacer" />
            <button type="button" className="ts-btn ts-btn--quiet" disabled={busy} onClick={onConnect}>
              {t("sync.remote.replaceAccess")}
            </button>
            <button
              type="button"
              className="ts-btn ts-btn--quiet"
              disabled={busy}
              title={t("sync.remote.disconnectHint")}
              onClick={() => void sync.run("connect", () => sync.client.disconnectRemote())}
            >
              {t("sync.remote.disconnect")}
            </button>
          </div>
        </>
      )}
    </section>
  );
}

// ── Syncthing ──────────────────────────────────────────────────────────────

function SyncthingSection({
  sync,
  overview,
}: {
  sync: SyncControl;
  overview: SyncOverview;
}): React.JSX.Element {
  const { t } = useI18n();
  const folder = overview.syncthing;
  const available = overview.capabilities.syncthing_status;
  const busy = sync.activity !== null;
  const [found, setFound] = useState<DiscoveredSyncthing | null>(null);
  const [searched, setSearched] = useState(false);
  const [manual, setManual] = useState(false);
  const [address, setAddress] = useState("127.0.0.1:8384");
  const [tls, setTls] = useState(false);
  const [apiKey, setApiKey] = useState("");

  const discover = async (): Promise<void> => {
    const result = await sync.run("connect", () => sync.client.syncthingDiscover());
    setFound(result ?? null);
    setSearched(true);
  };

  const connect = async (
    endpoint: DiscoveredSyncthing["endpoint"],
    key: string | null,
  ): Promise<void> => {
    await sync.run("connect", () => sync.client.syncthingConnect(endpoint, key));
    setFound(null);
    setManual(false);
    setApiKey("");
  };

  return (
    <section className="ts-sync__section" aria-label={t("sync.syncthing.title")}>
      <h2 className="ts-eyebrow">{t("sync.syncthing.title")}</h2>
      <p>{folder.detected ? t("sync.syncthing.detected") : t("sync.syncthing.notDetected")}</p>
      <p className="ts-dialog__hint">{describeLink(t, folder.link, overview.capabilities.server ?? false)}</p>

      {folder.link === "Connected" && (
        <>
          {folder.state !== undefined && (
            <p>
              {describeFolderState(t, folder.state)}
              {folder.progress !== undefined &&
                ` · ${t("sync.syncthing.progress", { percent: folder.progress })}`}
            </p>
          )}
          {folder.error !== undefined && <p className="ts-dialog__error">{folder.error}</p>}
          {folder.devices.length > 0 && (
            <ul className="ts-sync-list" aria-label={t("sync.syncthing.devices")}>
              {folder.devices.map((device) => (
                <li key={device.short_id} className="ts-sync-change">
                  <span className="ts-sync-list__title">{device.name}</span>
                  <span className="ts-sync-list__meta">
                    {device.connected
                      ? t("sync.syncthing.deviceConnected")
                      : t("sync.syncthing.deviceOffline")}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </>
      )}

      {/* Состояние Syncthing читается не во всякой оболочке; где нельзя —
          действия видны, но отключены, а причина уже названа строкой выше
          (FR-036). */}
      <div className="ts-sync__actions">
        <button
          type="button"
          className="ts-btn"
          disabled={!available || busy}
          title={available ? undefined : describeLink(t, "UnavailableInShell", overview.capabilities.server ?? false)}
          onClick={() => void discover()}
        >
          {t("sync.syncthing.find")}
        </button>
        <button
          type="button"
          className="ts-btn"
          disabled={!available || busy}
          title={available ? undefined : describeLink(t, "UnavailableInShell", overview.capabilities.server ?? false)}
          aria-expanded={manual}
          onClick={() => setManual((value) => !value)}
        >
          {t("sync.syncthing.manual")}
        </button>
        {folder.link !== "NotConfigured" && folder.link !== "UnavailableInShell" && (
          <button
            type="button"
            className="ts-btn ts-btn--quiet"
            disabled={busy}
            onClick={() => void sync.run("connect", () => sync.client.syncthingDisconnect())}
          >
            {t("sync.syncthing.disconnect")}
          </button>
        )}
      </div>

      {searched && found === null && (
        <p className="ts-dialog__hint" role="status">
          {t("sync.syncthing.notFound")}
        </p>
      )}
      {found !== null && (
        <div className="ts-sync__notice" role="group" aria-label={t("sync.syncthing.confirm")}>
          <p>{t("sync.syncthing.foundAt", { address: found.endpoint.address })}</p>
          <p className="ts-dialog__hint">{t("sync.syncthing.confirmHint")}</p>
          <div className="ts-sync__actions">
            <button
              type="button"
              className="ts-btn ts-btn--accent"
              disabled={busy}
              onClick={() => void connect(found.endpoint, null)}
            >
              {t("sync.syncthing.connect")}
            </button>
            <button type="button" className="ts-btn" onClick={() => setFound(null)}>
              {t("common.cancel")}
            </button>
          </div>
        </div>
      )}

      {manual && available && (
        <form
          className="ts-sync__form"
          onSubmit={(event) => {
            event.preventDefault();
            if (address.trim() !== "" && apiKey !== "") {
              void connect({ address: address.trim(), tls }, apiKey);
            }
          }}
        >
          <label className="ts-field">
            <span className="ts-field__label">{t("sync.syncthing.address")}</span>
            <input
              className="ts-field__input ts-field__input--code"
              value={address}
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setAddress(event.target.value)}
            />
            <span className="ts-field__hint">{t("sync.syncthing.addressHint")}</span>
          </label>
          <label>
            <input
              type="checkbox"
              className="ts-switch"
              checked={tls}
              onChange={(event) => setTls(event.target.checked)}
            />
            {t("sync.syncthing.tls")}
          </label>
          <label className="ts-field">
            <span className="ts-field__label">{t("sync.syncthing.apiKey")}</span>
            <input
              className="ts-field__input"
              type="password"
              value={apiKey}
              autoComplete="off"
              onChange={(event) => setApiKey(event.target.value)}
            />
          </label>
          <button
            type="submit"
            className="ts-btn ts-btn--accent"
            disabled={busy || address.trim() === "" || apiKey === ""}
          >
            {t("sync.syncthing.connect")}
          </button>
        </form>
      )}
    </section>
  );
}

// ── Конфликты ──────────────────────────────────────────────────────────────

function ConflictsSection({
  sync,
  overview,
  sectionRef,
  onMerge,
}: {
  sync: SyncControl;
  overview: SyncOverview;
  sectionRef: React.RefObject<HTMLElement | null>;
  onMerge: (conflict: Conflict) => void;
}): React.JSX.Element {
  const { t } = useI18n();
  const { client, revision } = sync;
  const [conflicts, setConflicts] = useState<Conflict[]>([]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const next = await client.conflicts();
        if (!cancelled) setConflicts(next);
      } catch {
        if (!cancelled) setConflicts([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, revision]);

  return (
    <section className="ts-sync__section" aria-label={t("sync.conflicts.title")} ref={sectionRef}>
      <h2 className="ts-eyebrow">
        {t("sync.conflicts.title")}
        {overview.conflicts > 0 && ` · ${overview.conflicts}`}
      </h2>
      {conflicts.length === 0 ? (
        <p className="ts-dialog__hint">{t("sync.conflicts.none")}</p>
      ) : (
        <ul className="ts-sync-list">
          {conflicts.map((conflict) => (
            <li key={conflict.id}>
              <button
                type="button"
                className="ts-sync-list__row"
                onClick={() => onMerge(conflict)}
              >
                <span className="ts-sync-list__title">{conflict.path}</span>
                <span className="ts-sync-list__meta">
                  {conflict.source.kind === "Pull"
                    ? t("sync.conflicts.fromPull")
                    : t("sync.conflicts.fromSyncthing", { device: conflict.source.device })}
                  {conflict.auto_merge?.kind === "Clean" && ` · ${t("sync.conflicts.autoClean")}`}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

// ── Хранитель ──────────────────────────────────────────────────────────────

function KeeperSection({
  sync,
  overview,
  i18n,
}: {
  sync: SyncControl;
  overview: SyncOverview;
  i18n: I18n;
}): React.JSX.Element {
  const { t } = i18n;
  const state = overview.keeper_state;
  const keeper = overview.keeper;
  const busy = sync.activity !== null;

  let status: string;
  switch (state.kind) {
    case "Unassigned":
      status = t("sync.keeper.unassigned");
      break;
    case "ThisDevice":
      status = t("sync.keeper.thisDevice");
      break;
    case "OtherDevice":
      status = t("sync.keeper.otherDevice", {
        device: keeper?.device_name ?? t("sync.keeper.unknown"),
      });
      break;
    case "Disputed":
      status = t("sync.keeper.disputed");
      break;
    default: {
      const exhaustive: never = state;
      status = exhaustive;
    }
  }

  return (
    <section className="ts-sync__section" aria-label={t("sync.keeper.title")}>
      <h2 className="ts-eyebrow">{t("sync.keeper.title")}</h2>
      <p className={state.kind === "Disputed" ? "ts-dialog__warn" : undefined}>{status}</p>
      <p className="ts-dialog__hint">
        {keeper !== undefined &&
          `${t("sync.keeper.since", { time: i18n.formatDate(new Date(keeper.since)) })} · `}
        {overview.last_snapshot_at === undefined
          ? t("sync.keeper.noSnapshots")
          : t("sync.keeper.lastSnapshot", {
              time: i18n.formatDate(new Date(overview.last_snapshot_at)),
            })}
      </p>
      <p className="ts-dialog__hint">{t("sync.keeper.explain")}</p>
      <div className="ts-sync__actions">
        {state.kind !== "ThisDevice" && (
          <button
            type="button"
            className="ts-btn"
            disabled={busy}
            onClick={() => void sync.run("resolve", () => sync.client.setKeeper())}
          >
            {t("sync.keeper.take")}
          </button>
        )}
        {keeper !== undefined && (
          <button
            type="button"
            className="ts-btn ts-btn--quiet"
            disabled={busy}
            onClick={() => void sync.run("resolve", () => sync.client.clearKeeper())}
          >
            {t("sync.keeper.clear")}
          </button>
        )}
      </div>
    </section>
  );
}

/**
 * Вход в редактор файлов правил (фича 012, FR-001). Раздел виден всегда:
 * правила можно править и до включения истории, и без Syncthing — действуют
 * ли они сейчас, скажет сам редактор.
 */
function RulesSection({ onOpen }: { onOpen: (file: IgnoreFile) => void }): React.JSX.Element {
  const { t } = useI18n();
  return (
    <section className="ts-sync__section" aria-label={t("sync.rules.title")}>
      <h2 className="ts-eyebrow">{t("sync.rules.title")}</h2>
      <p className="ts-dialog__hint">{t("sync.rules.explain")}</p>
      <ul className="ts-sync-rules-list">
        <li>
          <span>
            {t("sync.rules.gitignore")} — {t("sync.rules.gitignoreHint")}
          </span>
          <button type="button" className="ts-btn" onClick={() => onOpen("GitIgnore")}>
            {t("sync.rules.edit")}
          </button>
        </li>
        <li>
          <span>
            {t("sync.rules.stignore")} — {t("sync.rules.stignoreHint")}
          </span>
          <button type="button" className="ts-btn" onClick={() => onOpen("StIgnore")}>
            {t("sync.rules.edit")}
          </button>
        </li>
      </ul>
    </section>
  );
}

/**
 * Подтверждение действия на устройстве, которое не хранитель (FR-030).
 *
 * Живёт рядом с рабочей областью, а не внутри панели: снимок и обмен
 * вызываются и из палитры, при закрытой панели.
 */
export function KeeperConfirmDialog({
  request,
}: {
  request: KeeperConfirmation;
}): React.JSX.Element {
  const { t } = useI18n();
  useDismissOnEscape(request.cancel);
  return (
    <div
      className="ts-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-label={t("sync.keeper.confirmTitle")}
    >
      <div className="ts-dialog__head">
        <span className="ts-eyebrow">{t("sync.keeper.confirmTitle")}</span>
      </div>
      <div className="ts-dialog__body">
        <p>
          {request.keeper === null
            ? t("sync.keeper.confirmDisputed")
            : t("sync.keeper.confirmElsewhere", { device: request.keeper.device_name })}
        </p>
        <p className="ts-dialog__hint">{t("sync.keeper.confirmHint")}</p>
      </div>
      <div className="ts-dialog__foot">
        <button type="button" className="ts-btn" onClick={request.cancel}>
          {t("common.cancel")}
        </button>
        <span className="ts-spacer" />
        <button type="button" className="ts-btn ts-btn--danger" onClick={request.confirm}>
          {t("sync.keeper.confirmProceed")}
        </button>
      </div>
    </div>
  );
}
