import type { Translation } from "../define.js";
import { ruN } from "../define.js";
import type { sources as en } from "../en/sources.js";

export const sources: Translation<typeof en> = {
  "sources.command.insertCitation": "Вставить цитату…",
  "sources.command.toggle": "Источники",
  "sources.mode": "Источники",
  "sources.badge": (p) => ruN(p.count, "# запись", "# записи", "# записей"),
  "sources.problemsBadge": (p) => ruN(p.count, "# ошибка", "# ошибки", "# ошибок"),

  "sources.search.placeholder": "поиск по автору, году, ключу…",
  "sources.search.label": "Поиск по источникам",
  "sources.uncitedOnly": "Не цитируются",
  "sources.scan.progress": (p) => `цитаты: ${p.done} из ${p.total}`,
  "sources.scan.cancel": "отменить",
  "sources.scan.stopped": "проход остановлен",
  "sources.scan.resume": "продолжить",
  "sources.tag.uncited": "не цитируется",
  "sources.tag.broken": "битая",
  "sources.insertKey": (p) => `Вставить @${p.key}`,
  "sources.keyLabel": (p) => `ключ: ${p.key}`,
  "sources.inText": (p) => ruN(p.count, "в тексте # раз", "в тексте # раза", "в тексте # раз"),
  "sources.citing.title": "Цитируют",
  "sources.citing.none": "Этот источник пока не цитирует ни одна заметка.",
  "sources.missing.title": "Нет в файлах источников",
  "sources.missing.notes": (p) => ruN(p.count, "# заметка", "# заметки", "# заметок"),
  "sources.problems.title": "Ошибки в файлах",
  "sources.problems.place": (p) => (p.line > 0 ? `${p.file} · строка ${p.line}` : p.file),
  "sources.shown": (p) => `показаны ${p.shown} из ${p.total} — уточните запрос`,
  "sources.nothingFound": "Ничего не найдено.",

  "sources.empty.title": "В vault нет файлов источников",
  "sources.empty.create": "Создать references.bib",
  "sources.empty.pick": "Указать существующий…",
  "sources.empty.hint": "Положите файл .bib или .yml в папку заметок — он подхватится сам.",
  "sources.empty.readOnly": "Vault открыт только на чтение — файл не создать.",
  "sources.empty.noEntries": (p) => `В файлах источников нет записей: ${p.path}`,
  "sources.empty.created": "Файл пуст — наполните его в текстовом редакторе или менеджере литературы.",

  "sources.palette.label": "Вставить цитату",
  "sources.palette.placeholder": "автор, название, год или ключ",
  "sources.palette.footer": "↑↓ выбор · ↵ вставить · esc закрыть",

  "sources.plan.string": "Внутри строки цитату не вставить.",
  "sources.plan.comment": "Внутри комментария цитату не вставить.",
  "sources.plan.unknownKey": "Источника больше нет в файле — обновите список.",
  "sources.create.alreadyExists": "references.bib уже есть.",
  "sources.create.accessDenied": "Vault отказал в записи.",
  "sources.create.io": "Файл не создан.",

  "sources.offer.add": "В заметке нет списка литературы. Добавить?",
  "sources.offer.extend": (p) => `Файл ${p.file} не подключён к списку литературы. Добавить?`,
  "sources.offer.accept": "Добавить",
  "sources.offer.later": "Не сейчас",

  "sources.mark.missing": (p) => `Источника с ключом «${p.key}» нет.`,
  "sources.mark.ambiguous": (p) =>
    `Нет ни источника с ключом «${p.key}», ни метки с таким именем.`,
  "sources.status": (p) =>
    p.broken > 0
      ? `${ruN(p.citations, "# цитата", "# цитаты", "# цитат")} · ${ruN(p.broken, "# битая", "# битые", "# битых")}`
      : ruN(p.citations, "# цитата", "# цитаты", "# цитат"),

  "sources.problem.syntax": "запись не читается",
  "sources.problem.field": "у поля негодное значение",
  "sources.problem.duplicate": "ключ повторяется",
  "sources.problem.noType": "у записи нет вида",
  "sources.problem.unreadable": "файл не читается",
  "sources.problem.fileSyntax": "YAML-файл сломан",
  "sources.problem.tooMany": (p) => `и ещё ${p.hidden}`,
};
