import type {
  Change,
  FolderState,
  InProgressOp,
  LinkState,
  SyncError,
  SyncOverview,
  SyncProgress,
} from "../wasm-core/typsidian_core.js";
import type { Translate } from "../i18n/index.js";
import type { SyncActivity } from "./useSync.js";

// Слова для значений ядра (фича 011, FR-039). Разборы исчерпывающие, с веткой
// `never`: новый вид отказа или состояния в ядре без перевода — ошибка `tsc`,
// а не пустое место на экране. Текст собирается при показе (010, FR-014).

/** Отказ синхронизации словами. */
export function describeSyncError(t: Translate, error: SyncError): string {
  switch (error.kind) {
    case "Offline":
      return t("sync.error.offline");
    case "Timeout":
      return t("sync.error.timeout");
    case "AuthRequired":
      return t("sync.error.authRequired");
    case "AuthRejected":
      return t("sync.error.authRejected");
    case "NotFound":
      return t("sync.error.notFound");
    case "RejectedNonFastForward":
      return t("sync.error.rejectedNonFastForward");
    case "CorsBlocked":
      return t("sync.error.corsBlocked");
    case "UnsupportedScheme":
      return error.scheme === "internal"
        ? t("sync.error.internalRemote")
        : t("sync.error.unsupportedScheme", { scheme: error.scheme });
    case "InvalidRemoteUrl":
      return t("sync.error.invalidRemoteUrl");
    case "RemoteChanged":
      return t("sync.error.remoteChanged", { url: error.url });
    case "NoRemote":
      return t("sync.error.noRemote");
    case "KeeperElsewhere":
      return t("sync.error.keeperElsewhere", { device: error.keeper.device_name });
    case "KeeperDisputed":
      return t("sync.error.keeperDisputed");
    case "InProgressOperation":
      return t("sync.error.inProgressOperation");
    case "ExchangeRunning":
      return t("sync.error.exchangeRunning");
    case "RemoteUnavailable":
      return t("sync.error.remoteUnavailable");
    case "PullConflictsPending":
      return t("sync.error.pullConflictsPending");
    case "NoHistory":
      return t("sync.error.noHistory");
    case "HistoryExists":
      return t("sync.error.historyExists");
    case "ForeignRepository":
      return t("sync.error.foreignRepository");
    case "StIgnoreNotConfirmed":
      return t("sync.error.stIgnoreNotConfirmed");
    case "AuthorRequired":
      return t("sync.error.authorRequired");
    case "NotFlushed":
      return t("sync.error.notFlushed");
    case "NothingToSnapshot":
      return t("sync.error.nothingToSnapshot");
    case "ConflictNotFound":
      return t("sync.error.conflictNotFound");
    case "TargetNotEmpty":
      return t("sync.error.targetNotEmpty");
    case "Cancelled":
      return t("sync.error.cancelled");
    case "TooLarge":
      return t("sync.error.tooLarge", { limit: error.limit_mb });
    case "Io":
      return t("sync.error.io", { detail: error.message });
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

export function describeActivity(t: Translate, activity: SyncActivity): string {
  switch (activity) {
    case "enable":
      return t("sync.activity.enable");
    case "snapshot":
      return t("sync.activity.snapshot");
    case "push":
      return t("sync.activity.push");
    case "pull":
      return t("sync.activity.pull");
    case "connect":
      return t("sync.activity.connect");
    case "resolve":
      return t("sync.activity.resolve");
    default: {
      const exhaustive: never = activity;
      return exhaustive;
    }
  }
}

export function describeInProgress(t: Translate, op: InProgressOp): string {
  switch (op) {
    case "Merge":
      return t("sync.inProgress.merge");
    case "Rebase":
      return t("sync.inProgress.rebase");
    case "CherryPick":
      return t("sync.inProgress.cherryPick");
    case "Revert":
      return t("sync.inProgress.revert");
    case "Bisect":
      return t("sync.inProgress.bisect");
    case "Other":
      return t("sync.inProgress.other");
    default: {
      const exhaustive: never = op;
      return exhaustive;
    }
  }
}

export function describeFolderState(t: Translate, state: FolderState): string {
  switch (state) {
    case "Idle":
      return t("sync.syncthing.state.idle");
    case "Scanning":
      return t("sync.syncthing.state.scanning");
    case "Syncing":
      return t("sync.syncthing.state.syncing");
    case "Paused":
      return t("sync.syncthing.state.paused");
    case "Error":
      return t("sync.syncthing.state.error");
    default: {
      const exhaustive: never = state;
      return exhaustive;
    }
  }
}

/** `server` — ядро работает на сервере (038): там Syncthing не поддерживается вовсе. */
export function describeLink(t: Translate, link: LinkState, server = false): string {
  switch (link) {
    case "NotConfigured":
      return t("sync.syncthing.link.notConfigured");
    case "Unreachable":
      return t("sync.syncthing.link.unreachable");
    case "Unauthorized":
      return t("sync.syncthing.link.unauthorized");
    case "UnavailableInShell":
      return server
        ? t("sync.syncthing.link.unavailableOnServer")
        : t("sync.syncthing.link.unavailableInShell");
    case "FolderNotShared":
      return t("sync.syncthing.link.folderNotShared");
    case "CertificateChanged":
      return t("sync.syncthing.link.certificateChanged");
    case "Connected":
      return t("sync.syncthing.link.connected");
    default: {
      const exhaustive: never = link;
      return exhaustive;
    }
  }
}

/** Вид изменения одним словом — для списка изменений и истории. */
export function describeChange(t: Translate, change: Change["change"]): string {
  switch (change.kind) {
    case "Added":
      return t("sync.change.added");
    case "Modified":
      return t("sync.change.modified");
    case "Deleted":
      return t("sync.change.deleted");
    case "Renamed":
      return t("sync.change.renamed", { from: change.from });
    default: {
      const exhaustive: never = change;
      return exhaustive;
    }
  }
}

/** Тон индикатора: спокойный, требует внимания, отказ. */
export type IndicatorTone = "idle" | "busy" | "attention" | "error";

/**
 * Краткое состояние для индикатора (FR-035). Порядок — от того, что требует
 * действия, к спокойному: отказ, конфликты, спор хранителей, идущая операция,
 * доставка Syncthing, неотправленное и неснятое.
 */
export function indicatorText(
  t: Translate,
  overview: SyncOverview | null,
  activity: SyncActivity | null,
  error: SyncError | null,
): { label: string; tone: IndicatorTone } {
  if (activity !== null) return { label: describeActivity(t, activity), tone: "busy" };
  if (overview === null) return { label: t("sync.indicator.loading"), tone: "idle" };
  if (error !== null) {
    return {
      label: error.kind === "Offline" ? t("sync.indicator.offline") : t("sync.indicator.failed"),
      tone: "error",
    };
  }
  if (overview.conflicts > 0) {
    return { label: t("sync.indicator.conflicts", { count: overview.conflicts }), tone: "attention" };
  }
  if (overview.keeper_state.kind === "Disputed") {
    return { label: t("sync.indicator.keeperDisputed"), tone: "attention" };
  }
  if (overview.ignore_warnings.length > 0) {
    return { label: t("sync.indicator.ignoreWarning"), tone: "attention" };
  }
  const syncthing = overview.syncthing;
  if (syncthing.state === "Syncing" || syncthing.state === "Scanning") {
    return {
      label: t("sync.indicator.syncthing", { percent: syncthing.progress ?? 0 }),
      tone: "busy",
    };
  }
  const history = overview.history;
  switch (history.kind) {
    case "Absent":
      return {
        label: syncthing.detected
          ? t("sync.indicator.syncthingNoHistory")
          : t("sync.indicator.noHistory"),
        tone: "idle",
      };
    case "Lost":
      return { label: t("sync.indicator.historyLost"), tone: "attention" };
    case "ForeignParent":
      return { label: t("sync.indicator.foreign"), tone: "idle" };
    case "Mirror":
      return { label: t("sync.indicator.mirror"), tone: "idle" };
    case "Local": {
      if (history.in_progress !== undefined) {
        return { label: describeInProgress(t, history.in_progress), tone: "attention" };
      }
      const ahead = overview.remote?.ahead ?? 0;
      const behind = overview.remote?.behind ?? 0;
      const parts: string[] = [];
      if (history.changes > 0) parts.push(t("sync.indicator.changes", { count: history.changes }));
      if (ahead > 0 || behind > 0) parts.push(t("sync.indicator.aheadBehind", { ahead, behind }));
      if (parts.length === 0) return { label: t("sync.indicator.upToDate"), tone: "idle" };
      return { label: parts.join(" · "), tone: "idle" };
    }
    default: {
      const exhaustive: never = history;
      return exhaustive;
    }
  }
}

/** «получение: 12 из 340» — ход массовой операции (T075, принцип X). */
export function describeProgress(t: Translate, progress: SyncProgress): string {
  const operation = (() => {
    switch (progress.operation) {
      case "FirstSnapshot":
        return t("sync.progress.firstSnapshot");
      case "Pull":
        return t("sync.progress.pull");
      case "Push":
        return t("sync.progress.push");
      case "Clone":
        return t("sync.progress.clone");
      case "Resolve":
        return t("sync.progress.resolve");
    }
  })();
  return t("sync.progress", { operation, done: progress.done, total: progress.total });
}
