import type { Translation } from "../define.js";
import { ruN } from "../define.js";
import { formatNumber } from "../../format.js";

const num = (value: number): string => formatNumber("ru", value);
import type { importing as en } from "../en/import.js";

// Импорт из Obsidian (фича 021): диалог C1/C4/C5, карточка хода C2, предложение
// снимка и тексты заметки отчёта C3 (уходят в ядро через `ImportJob.texts`).
export const importing: Translation<typeof en> = {
  "import.command": "Импорт из Obsidian…",
  "import.pickTitle": "Выберите папку с заметками Obsidian",
  "import.title": "Импорт из Obsidian",

  // ── C1: настройка ──────────────────────────────────────────────────
  "import.source.label": "Откуда — папка с заметками Markdown",
  "import.source.none": "Папка не выбрана",
  "import.source.choose": "Выбрать…",
  "import.source.again": "Выбрать другую…",
  "import.survey.reading": "Считаю файлы…",
  "import.survey.notes": (p) => ruN(p.count, "заметка", "заметки", "заметок"),
  "import.survey.images": (p) => ruN(p.count, "картинка", "картинки", "картинок"),
  "import.survey.other": (p) => ruN(p.count, "прочий файл", "прочих файла", "прочих файлов"),
  "import.survey.excluded": "без .obsidian/ и .trash/",
  "import.dest.label": "Куда — папка в vault",
  "import.dest.root": "корень vault",
  "import.dest.default": "из-obsidian",
  "import.dest.change": "Изменить",
  "import.dest.done": "Готово",
  "import.dest.input": "Папка назначения в vault",
  "import.dest.hint":
    "Картинки лягут по правилам вложений vault (016). Исходная папка не меняется.",
  "import.conflict.label": "Если файл с таким именем уже есть",
  "import.conflict.skip": "Пропустить",
  "import.conflict.skipHint":
    "Существующий файл остаётся, заметка попадёт в отчёт как пропущенная",
  "import.conflict.beside": "Положить рядом",
  "import.conflict.besideHint": "Под именем «имя (импорт).typ»",
  "import.other.copy": "Копировать прочие файлы как есть",
  "import.other.copyHint": (p) =>
    `PDF, аудио, архивы — ${num(p.count)}. Иначе они попадут в отчёт как пропущенные`,
  "import.start": (p) =>
    `Импортировать ${ruN(p.count, "# заметку", "# заметки", "# заметок")}`,

  // ── C4: продолжение ────────────────────────────────────────────────
  "import.resume.intro": (p) =>
    `Эту папку уже импортировали сюда ${p.when}; импорт остановился на ${num(p.done)} / ${num(p.total)}.`,
  "import.resume.hint": (p) =>
    `Перенесённое раньше не повторится: ${num(p.done)} уже на месте, будет перенесено ${num(p.left)}. Ссылки между старыми и новыми заметками сойдутся.`,
  "import.resume.start": (p) => `Продолжить импорт — ${num(p.count)}`,
  "import.resume.fresh": "Выбрать другую папку…",

  // ── C5: нет доступа к папкам ───────────────────────────────────────
  "import.unavailable":
    "Этот браузер не даёт выбрать папку на диске, поэтому импорт недоступен. Откройте Typsidian в Chrome или Edge либо в приложении для компьютера.",
  "import.noVault": "Сначала откройте vault: импорт кладёт заметки в него.",

  // ── Отказы ─────────────────────────────────────────────────────────
  "import.error.open": "Папку не удалось открыть.",
  "import.error.NoSource": "Исходная папка больше не открыта. Выберите её снова.",
  "import.error.SessionLost": "Сеанс импорта потерян.",
  "import.error.AccessDenied": "Нет доступа к папке или к vault.",
  "import.error.Io": "Ошибка диска прервала импорт.",
  "import.error.generic": (p) => `Импорт не удался: ${p.detail}`,

  // ── C2: ход ────────────────────────────────────────────────────────
  "import.progress.counter": (p) => `${num(p.done)} / ${num(p.total)}`,
  "import.progress.cancel": "Отменить",
  "import.progress.cancelling": "Отменяю…",
  "import.progress.minimize": "Свернуть в строку статуса",
  "import.progress.expand": "Показать ход импорта",
  "import.progress.label": (p) => `Ход импорта: ${num(p.done)} из ${num(p.total)}`,
  "import.progress.clean": (p) => `${num(p.count)} чисто`,
  "import.progress.lossy": (p) => `${num(p.count)} с потерями`,
  "import.progress.skipped": (p) => `${num(p.count)} пропущено`,
  "import.progress.resumed": (p) => `${num(p.count)} перенесено раньше`,
  "import.pill": (p) => `импорт ${num(p.done)} / ${num(p.total)}`,

  // ── Итог ───────────────────────────────────────────────────────────
  "import.done.title": "Импорт завершён",
  "import.done.cancelled": "Импорт отменён",
  "import.done.empty": "Импортировать было нечего.",
  "import.done.openReport": "Открыть отчёт",
  "import.done.reportFailed": "Заметка отчёта не записана.",
  "import.done.resumeHint": "Запустите импорт с теми же папками ещё раз, чтобы продолжить.",
  "import.done.problems": "Стоит посмотреть",
  "import.done.snapshotOffer":
    "Сделать снимок истории, чтобы откатить импортированные заметки одним шагом?",
  "import.done.snapshot": "Сделать снимок",
  "import.done.snapshotTaken": "Снимок сделан.",
  "import.done.snapshotFailed": "Снимок не сделан.",
  "import.done.snapshotMessage": "Импорт из Obsidian",
  "import.done.close": "Закрыть",

  // ── Тексты заметки отчёта (C3) ─────────────────────────────────────
  "import.report.fileName": (p) => `импорт-${p.stamp}`,
  "import.report.title": (p) => `Импорт из Obsidian — ${p.when}`,
  "import.report.source": "Откуда",
  "import.report.destination": "Куда",
  "import.report.summary": "Итог",
  "import.report.clean": "Перенесено чисто",
  "import.report.lossy": "С потерями",
  "import.report.skipped": "Пропущено",
  "import.report.resumed": "Перенесено раньше",
  "import.report.cancelled": "Импорт отменён до конца.",
  "import.report.lossyHeading": "С потерями",
  "import.report.skippedHeading": "Пропущено",
  "import.report.renamedHeading": "Имена изменены",
  "import.report.line": "строка {n}",
  "import.report.lossHtml": "HTML оставлен исходным текстом",
  "import.report.lossEmbed": "Встраивание заменено ссылкой или оставлено текстом",
  "import.report.lossPluginBlock": "Блок плагина (Dataview, Tasks…) оставлен текстом",
  "import.report.lossDiagram": "Диаграмма (mermaid…) оставлена текстом",
  "import.report.lossMath": "Формула оставлена текстом",
  "import.report.lossSectionLink": "Ссылка на раздел ведёт на заметку целиком",
  "import.report.lossBlockLink": "Ссылка на блок ведёт на заметку целиком",
  "import.report.lossMissingTarget": "Цель ссылки не найдена",
  "import.report.lossAmbiguousTarget": "Имя есть у нескольких заметок; взята ближайшая",
  "import.report.lossProperties": "Шапка свойств оставлена комментарием",
  "import.report.lossSyntax": "Заметка оставлена исходным текстом",
  "import.report.skipTooLarge": "файл больше 5 МБ",
  "import.report.skipNotUtf8": "текст не в UTF-8",
  "import.report.skipOtherFile": "не Markdown и не картинка; копирование прочих файлов выключено",
  "import.report.skipExistsSkipped": "файл с таким именем уже есть",
  "import.report.skipUnreadable": "файл не удалось прочитать",
  "import.report.skipWriteFailed": "заметку не удалось записать",
};
