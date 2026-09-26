import type { InstallError } from "../wasm-core/typsidian_core.js";

/**
 * Причина отказа установки внешнего шаблона словами (фича 007, FR-007, SC-008).
 *
 * Отказ приходит размеченным объединением `{kind, …}` — одинаково с web и с
 * desktop (`Failure` в оболочке сериализуется без обёртки). Решения здесь нет:
 * какая причина наступила, определило ядро, а здесь только формулировка. Общего
 * «не удалось» нет ни у одной ветки — каждая причина чинится по-своему, и
 * пользователь обязан понимать, чем именно.
 */
export function describeInstallError(cause: unknown): string {
  if (!isInstallError(cause)) {
    if (isKind(cause, "NoVaultOpen")) return "Папка заметок не открыта.";
    return `Шаблон не установлен: ${String(cause)}`;
  }
  switch (cause.kind) {
    case "Offline":
      return `Сети нет: ${cause.url} недостижим.`;
    case "SourceUnavailable":
      return `Источник ${cause.url} ответил отказом.`;
    case "Blocked":
      // FR-015: отдельная причина и подсказка, что делать без сети.
      return (
        `Обращение к ${cause.url} запрещено средой этого клиента. ` +
        "Шаблон можно положить вручную в папку .typsidian/packages/ этой папки заметок."
      );
    case "VersionNotFound":
      // FR-017: чинится правкой версии в заметке, а не сетью.
      return `В источнике нет такой версии (${cause.spec}) — проверьте версию в подключении.`;
    case "TimedOut":
      return `Ожидание ответа от ${cause.url} исчерпано.`;
    case "ArchiveRejected":
      return `Архив отклонён, ни одного файла не создано: ${cause.reason}.`;
    case "NotWritable":
      return "Папка заметок недоступна на запись — установить шаблон некуда.";
    case "AutoDownloadDisabled":
      return `Шаблон ${cause.spec} не установлен, а автозагрузка выключена.`;
  }
}

const KINDS: ReadonlySet<string> = new Set<InstallError["kind"]>([
  "Offline",
  "SourceUnavailable",
  "Blocked",
  "VersionNotFound",
  "TimedOut",
  "ArchiveRejected",
  "NotWritable",
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
