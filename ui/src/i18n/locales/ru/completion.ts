import type { Translation } from "../define.js";
import { ruN } from "../define.js";
import type { completion as en } from "../en/completion.js";

export const completion: Translation<typeof en> = {
  "completion.command.trigger": "Вызвать дополнение",
  "completion.command.goto": "Перейти к определению",
  "completion.label": "Предложения",

  "completion.group.labels": "Метки",
  "completion.group.sources": "Источники",
  "completion.group.notes": "Заметки",
  "completion.group.head": (p) => `${p.title} · ${p.count}`,
  "completion.group.more": (p) => `${p.shown} из ${p.total} — уточните набор`,

  "completion.kind.heading": "заголовок",
  "completion.kind.figure": "рисунок",
  "completion.kind.table": "таблица",
  "completion.kind.equation": "формула",
  "completion.kind.other": "метка",
  "completion.kind.source": "источник",
  "completion.kind.note": "заметка",
  "completion.duplicate": "объявлена дважды",
  "completion.thisFolder": "эта папка",
  "completion.rootFolder": "корень vault",

  "completion.footer": "↑↓ выбор · ↵ / ⇥ вставить · esc закрыть",
  "completion.willBe": (p) => `будет: ${p.text}`,
  "completion.status": (p) => ruN(p.count, "# предложение", "# предложения", "# предложений"),

  "completion.setting.auto": "Предлагать метки и заметки при наборе",
  "completion.setting.autoHint": (p) =>
    p.shortcut === ""
      ? "Выключено — список открывается только из палитры команд."
      : `Выключено — список открывается только по ${p.shortcut} или из палитры команд.`,

  "completion.goto.noReference": "Под кареткой нет ссылки.",
  "completion.goto.labelMissing": (p) => `Метки <${p.name}> нет ни в заметке, ни в связанных файлах.`,
  "completion.goto.labelFromBuild": (p) => `<${p.name}> создаётся кодом — места в тексте у неё нет.`,
  "completion.goto.noteMissing": (p) => `Заметки «${p.target}» нет.`,
  "completion.goto.outsideVault": (p) => `«${p.target}» ведёт за пределы папки vault.`,
  "completion.goto.source": (p) => `@${p.key} — источник, он показан в «Источниках».`,

  "completion.mark.missingLabel": (p) => `Метки <${p.name}> нет`,
  "completion.mark.missingLabelDetail": "Она не объявлена ни в заметке, ни в связанных файлах, ни в источниках.",
  "completion.mark.missingNote": (p) => `Заметки «${p.target}» нет`,
  "completion.mark.missingNoteDetail": (p) => `Её можно создать: ${p.path}.`,
  "completion.mark.createNote": "Создать заметку",
  "completion.mark.createFailed": "Заметку создать не удалось.",
  "completion.mark.outside": (p) => `«${p.target}» ведёт за пределы vault`,
  "completion.mark.outsideDetail": "Ссылка может вести только к заметке внутри папки vault.",
  "completion.mark.duplicate": (p) => `Метка <${p.name}> объявлена повторно`,
  "completion.mark.duplicateDetail": (p) => `Ещё: ${p.places}.`,
  "completion.mark.line": (p) => `строка ${p.line}`,
  "completion.mark.fileLine": (p) => `${p.path}, строка ${p.line}`,
  "completion.mark.count": (p) => ruN(p.count, "# отметка", "# отметки", "# отметок"),
};
