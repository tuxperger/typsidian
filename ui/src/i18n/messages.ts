/**
 * Сообщения ядра на языке интерфейса (фича 010, FR-002, FR-004, FR-014).
 *
 * Ядро отдаёт вид сообщения и параметры (`AppMessage`), текст собирается здесь.
 * Разбор исчерпывающий, с веткой `never`: новый вид в ядре без перевода —
 * ошибка `tsc`, а не пустое место на экране.
 *
 * Текст вычисляется при показе, а не хранится: показанное сообщение
 * перерисовывается на новом языке без повторения действия (FR-014).
 */

import type {
  AppMessage,
  ArchiveProblem,
  Diagnostic,
  FingerprintActual,
  FontProblem,
  InstallError,
  ResolveReason,
} from "../wasm-core/typsidian_core.js";
import type { Translate } from "./index.js";

/** Сообщение приложения словами. */
export function message(t: Translate, m: AppMessage): string {
  switch (m.code) {
    case "DependencyUnreadable":
      return t("app.dependencyUnreadable", { path: m.path, reason: resolveReason(t, m.reason) });
    case "TemplateMissing":
      return t("app.templateMissing", { spec: m.spec });
    case "TemplateDownloaded":
      return t("app.templateDownloaded", {
        spec: m.spec,
        url: m.url,
        fileCount: m.fileCount,
        fingerprint: m.fingerprint,
      });
    case "TemplateInstallFailed":
      return t("app.templateInstallFailed", {
        spec: m.spec,
        reason: installFailure(t, m.reason),
      });
    case "TemplateFingerprintDiverged":
      return t("app.templateFingerprintDiverged", {
        spec: m.spec,
        recorded: m.recorded,
        actual: fingerprintActual(t, m.actual),
      });
    case "VaultFontUnusable":
      return t("app.vaultFontUnusable", { path: m.path, reason: fontProblem(t, m.reason) });
    case "NoteOutsideVault":
      return t("app.noteOutsideVault", { path: m.path });
    default: {
      const exhaustive: never = m;
      return exhaustive;
    }
  }
}

/**
 * Текст замечания сборки: сообщение приложения — на языке интерфейса,
 * сообщение Typst — как есть (FR-004).
 */
export function diagnosticText(t: Translate, d: Diagnostic): string {
  return d.app ? message(t, d.app) : d.message;
}

/** Почему шаблон не установился — одна формулировка для замечания и для явной загрузки. */
export function installFailure(t: Translate, e: InstallError): string {
  switch (e.kind) {
    case "Offline":
      return t("app.install.offline", { url: e.url });
    case "SourceUnavailable":
      return t("app.install.sourceUnavailable", { url: e.url });
    case "Blocked":
      return t("app.install.blocked", { url: e.url });
    case "VersionNotFound":
      return t("app.install.versionNotFound", { spec: e.spec });
    case "TimedOut":
      return t("app.install.timedOut", { url: e.url });
    case "ArchiveRejected":
      return t("app.install.archiveRejected", { reason: archiveProblem(t, e.reason) });
    case "NotWritable":
      return t("app.install.notWritable");
    case "LocalWriteFailed":
      return t("app.install.localWriteFailed");
    case "AutoDownloadDisabled":
      return t("app.install.autoDownloadDisabled", { spec: e.spec });
    default: {
      const exhaustive: never = e;
      return exhaustive;
    }
  }
}

function resolveReason(t: Translate, reason: ResolveReason): string {
  switch (reason) {
    case "notFound":
      return t("app.resolve.notFound");
    case "unreadable":
      return t("app.resolve.unreadable");
    case "accessDenied":
      return t("app.resolve.accessDenied");
    case "outsideVault":
      return t("app.resolve.outsideVault");
    case "tooLarge":
      return t("app.resolve.tooLarge");
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}

function fontProblem(t: Translate, problem: FontProblem): string {
  switch (problem.kind) {
    case "unreadable":
      return resolveReason(t, problem.reason);
    case "invalid":
      return t("app.font.invalid");
    default: {
      const exhaustive: never = problem;
      return exhaustive;
    }
  }
}

function fingerprintActual(t: Translate, actual: FingerprintActual): string {
  switch (actual.kind) {
    case "digest":
      return t("app.fingerprint.digest", { digest: actual.digest });
    case "fileMissing":
      return t("app.fingerprint.fileMissing", { name: actual.name });
    default: {
      const exhaustive: never = actual;
      return exhaustive;
    }
  }
}

function archiveProblem(t: Translate, problem: ArchiveProblem): string {
  switch (problem.problem) {
    case "tooLarge":
      return t("app.archive.tooLarge");
    case "unparsable":
      return t("app.archive.unparsable");
    case "entryWithoutPath":
      return t("app.archive.entryWithoutPath");
    case "entryUnreadable":
      return t("app.archive.entryUnreadable");
    case "unpackedTooLarge":
      return t("app.archive.unpackedTooLarge");
    case "empty":
      return t("app.archive.empty");
    case "noManifest":
      return t("app.archive.noManifest");
    case "pathNotText":
      return t("app.archive.pathNotText");
    case "escapesFolder":
      return t("app.archive.escapesFolder", { entry: problem.entry });
    case "entryWithoutName":
      return t("app.archive.entryWithoutName");
    default: {
      const exhaustive: never = problem;
      return exhaustive;
    }
  }
}
