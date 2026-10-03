import { ruN, type Translation } from "../define.js";
import type { app as en } from "../en/app.js";

export const app: Translation<typeof en> = {
  "app.dependencyUnreadable": (p) => `${p.path}: ${p.reason}`,
  "app.templateMissing": (p) => `шаблон ${p.spec} не установлен в папке заметок`,
  "app.templateDownloaded": (p) =>
    `Шаблон ${p.spec} загружен с ${p.url} (${ruN(p.fileCount, "# файл", "# файла", "# файлов")}, отпечаток ${p.fingerprint.slice(0, 12)}).`,
  "app.templateInstallFailed": (p) => `Шаблон ${p.spec} не установлен: ${p.reason}`,
  "app.templateFingerprintDiverged": (p) =>
    `содержимое шаблона ${p.spec} разошлось с записанным отпечатком (${p.actual} вместо ${p.recorded.slice(0, 12)}); приложение его не переустанавливает`,
  "app.vaultFontUnusable": (p) => `шрифт ${p.path}: ${p.reason}`,
  "app.noteOutsideVault": (p) => `${p.path}: заметка вне папки vault`,
  "app.resolve.notFound": "файл не найден",
  "app.resolve.unreadable": "файл не читается",
  "app.resolve.accessDenied": "нет прав на чтение",
  "app.resolve.outsideVault": "файл вне папки vault",
  "app.resolve.tooLarge": "файл больше поддерживаемого размера",
  "app.font.invalid": "файл не разбирается как шрифт",
  "app.fingerprint.digest": (p) => p.digest.slice(0, 12),
  "app.fingerprint.fileMissing": (p) => `файла ${p.name} больше нет`,
  "app.install.offline": (p) => `Сети нет: ${p.url} недостижим.`,
  "app.install.sourceUnavailable": (p) => `Источник ${p.url} ответил отказом.`,
  "app.install.blocked": (p) =>
    `Обращение к ${p.url} запрещено средой этого клиента. Шаблон можно положить вручную в папку .typsidian/packages/ этой папки заметок.`,
  "app.install.versionNotFound": (p) =>
    `В источнике нет такой версии (${p.spec}) — проверьте версию в подключении.`,
  "app.install.timedOut": (p) => `Ожидание ответа от ${p.url} исчерпано.`,
  "app.install.archiveRejected": (p) => `Архив отклонён, ни одного файла не создано: ${p.reason}.`,
  "app.install.notWritable": "Папка заметок недоступна на запись — установить шаблон некуда.",
  "app.install.localWriteFailed": "Файлы шаблона не удалось записать в папку заметок.",
  "app.install.autoDownloadDisabled": (p) => `Шаблон ${p.spec} не установлен, а автозагрузка выключена.`,
  "app.install.noVaultOpen": "Папка заметок не открыта.",
  "app.install.unknown": (p) => `Шаблон не установлен: ${p.detail}`,
  "app.archive.tooLarge": "архив больше поддерживаемого размера",
  "app.archive.unparsable": "архив не разбирается",
  "app.archive.entryWithoutPath": "запись архива не имеет пригодного пути",
  "app.archive.entryUnreadable": "запись архива не читается",
  "app.archive.unpackedTooLarge": "распакованный шаблон больше поддерживаемого размера",
  "app.archive.empty": "в архиве нет файлов",
  "app.archive.noManifest": "в архиве нет манифеста typst.toml",
  "app.archive.pathNotText": "путь записи архива не является текстом",
  "app.archive.escapesFolder": (p) => `запись «${p.entry}» ведёт за пределы папки шаблона`,
  "app.archive.entryWithoutName": "запись архива без имени",
};
