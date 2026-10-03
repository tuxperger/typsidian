import type { InstallError } from "../wasm-core/typsidian_core.js";
import type { Phrase } from "../i18n/index.js";
import { installFailure } from "../i18n/messages.js";

/**
 * Причина отказа установки внешнего шаблона словами (фича 007, FR-007, SC-008).
 *
 * Отказ приходит размеченным объединением `{kind, …}` — одинаково с web и с
 * desktop (`Failure` в оболочке сериализуется без обёртки). Решения здесь нет:
 * какая причина наступила, определило ядро, а здесь только формулировка. Общего
 * «не удалось» нет ни у одной ветки — каждая причина чинится по-своему, и
 * пользователь обязан понимать, чем именно.
 */
export function describeInstallError(cause: unknown): Phrase {
  if (!isInstallError(cause)) {
    if (isKind(cause, "NoVaultOpen")) return (t) => t("app.install.noVaultOpen");
    const detail = String(cause);
    return (t) => t("app.install.unknown", { detail });
  }
  return (t) => installFailure(t, cause);
}

const KINDS: ReadonlySet<string> = new Set<InstallError["kind"]>([
  "Offline",
  "SourceUnavailable",
  "Blocked",
  "VersionNotFound",
  "TimedOut",
  "ArchiveRejected",
  "NotWritable",
  "LocalWriteFailed",
  "AutoDownloadDisabled",
]);

function isInstallError(cause: unknown): cause is InstallError {
  return (
    typeof cause === "object" &&
    cause !== null &&
    "kind" in cause &&
    typeof cause.kind === "string" &&
    KINDS.has(cause.kind)
  );
}

function isKind(cause: unknown, kind: string): boolean {
  return (
    typeof cause === "object" && cause !== null && "kind" in cause && cause.kind === kind
  );
}
