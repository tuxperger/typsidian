import type { Translation } from "../define.js";
import { ruN } from "../define.js";
import type { attachments as en } from "../en/attachments.js";

export const attachments: Translation<typeof en> = {
  "attachments.command.insertImage": "Вставить изображение…",
  "attachments.insertMenu.fromFile": "Изображение из файла…",
  "attachments.status.inserted": (p) => `вставлено: ${p.name}`,
  "attachments.unnamed": "изображение из буфера",

  "attachments.refused.title": "Изображение не вставлено",
  "attachments.refused.unchanged": "Текст заметки не изменён.",
  "attachments.refused.readOnly": "Vault открыт только на чтение.",
  "attachments.refused.notANote": "Изображение вставляется только в заметку Typst.",
  "attachments.refused.busy": "Дождитесь окончания текущей вставки.",
  "attachments.refused.reason": (p) => `${p.name}: ${p.reason}`,

  "attachments.problem.accessDenied": "vault отказал в записи",
  "attachments.problem.io": "файл не записан — проверьте свободное место",
  "attachments.problem.nameBusy": "в папке вложений не нашлось свободного имени",
  "attachments.problem.sourceUnreadable": "исходный файл не прочитался",
  "attachments.problem.skipped": "вид файла не поддерживается",

  "attachments.progress.title": (p) => `Копирую вложения: ${p.done} из ${p.total}`,
  "attachments.progress.current": (p) => `${p.name} → ${p.folder}/`,
  "attachments.progress.cancel": "Отменить",
  "attachments.progress.note": "уже скопированные останутся и будут вставлены",

  "attachments.summary.title": (p) => `Вставлено ${p.inserted} из ${p.total}`,
  "attachments.summary.reused": "уже лежит в vault, копия не создана",
  "attachments.summary.savedAs": (p) => `сохранён как ${p.name}`,
  "attachments.summary.skipped": (p) => `Пропущено — вид не поддерживается: ${p.names}`,
  "attachments.summary.declined": (p) =>
    ruN(
      p.count,
      "# большое изображение не вставлено",
      "# больших изображения не вставлено",
      "# больших изображений не вставлено",
    ),
  "attachments.summary.cancelled": (p) => `отменено: ${p.count}`,
  "attachments.summary.notInserted": "Заметка закрыта — файлы сохранены, но не вставлены.",
  "attachments.summary.dismiss": "Закрыть",

  "attachments.warn.history": "Папка вложений не попадает в историю",
  "attachments.warn.delivery": "Папка вложений не доставляется на другие устройства",
  "attachments.warn.detail": (p) =>
    `${p.folder}/ исключена правилами ${p.files}. Файл всё равно сохранён.`,
  "attachments.warn.openRules": "Открыть правила",

  "attachments.link.dropped":
    "Перетащена ссылка, а не файл — сохраните изображение и перетащите файл.",

  "attachments.large.title": "Вставить большое изображение?",
  "attachments.large.asIs": "Будет скопирован как есть, без сжатия.",
  "attachments.large.target": (p) => `в папку ${p.folder}/`,
  "attachments.large.cancel": "Отмена",
  "attachments.large.insert": "Вставить",

  "attachments.drop.indicator": (p) =>
    `${ruN(p.count, "# изображение", "# изображения", "# изображений")} — сюда, строка ${p.line}`,
  "attachments.drop.tag": "перетаскивание",

  "attachments.sheet.title": "Вставить изображение",
  "attachments.sheet.camera": "Снять фото",
  "attachments.sheet.gallery": "Выбрать из галереи",
  "attachments.sheet.files": "Выбрать файл…",
  "attachments.sheet.close": "Закрыть",
};
