import { ruN, type Translation } from "../define.js";
import type { tree as en } from "../en/tree.js";

export const tree: Translation<typeof en> = {
  "tree.access.settingsLost":
    "Служебная папка этого vault исчезла. Настройки уровня папки и локально сохранённые шаблоны Typst утрачены — заметки, доски и файлы вставок при этом на месте, а кэш пересоберётся сам.",
  "tree.access.settingsReadOnly":
    "Vault недоступен на запись, поэтому настройки уровня папки менять некуда: расположение файлов вставок и пометки разделов сохранить не получится.",
  "tree.access.readOnly":
    "Права на запись в vault отозваны. Заметки открываются и читаются как обычно, но сохранение, создание, переименование и удаление сейчас отклоняются.",
  "tree.access.unavailableTitle": "Vault недоступен",
  "tree.access.unavailableLead":
    "Vault стал недоступен — например, отключён диск или отозван доступ к папке. Это ",
  "tree.access.unavailableNot": "не",
  "tree.access.unavailableTail": " означает, что заметки удалены.",
  "tree.access.unsavedKept":
    "Несохранённые правки открытых заметок остаются в памяти, пока приложение работает: если закрыть его до восстановления доступа, они будут потеряны.",
  "tree.access.reopen": "Открыть vault заново",

  "tree.backlinks.deleteConsequence": (p) =>
    `После удаления «${p.name}» ${ruN(p.count, "# заметка", "# заметки", "# заметок")} останутся с битой ссылкой:`,
  "tree.backlinks.renameConsequence": (p) =>
    `После переименования «${p.name}» ${ruN(p.count, "# заметка", "# заметки", "# заметок")} продолжат ссылаться на старое имя, и эти ссылки станут битыми:`,
  "tree.backlinks.deleteAnyway": "Всё равно удалить",
  "tree.backlinks.renameAnyway": "Всё равно переименовать",
  "tree.backlinks.dialogLabel": (p) => `Ссылки на заметку ${p.name}`,
  "tree.backlinks.affectedLabel": "Заметки, чьи ссылки станут битыми",
  "tree.backlinks.confirmGroup": "Подтверждение операции",
  "tree.brokenLinks": (p) => `Битые ссылки: ${p.targets}`,
  "tree.bulkChange": "Идёт массовое обновление vault — дерево приводится в соответствие с диском.",
  "tree.emptyNotes": "Vault пуст — создайте первую заметку.",
  "tree.label": "Дерево заметок",

  "tree.create.note": "Заметка Typst",
  "tree.create.noteHint": ".typ · компилируется в PDF",
  "tree.create.board": "Доска Excalidraw",
  "tree.create.boardHint": ".excalidraw · встроенный",
  "tree.create.button": "Создать файл",
  "tree.create.menuLabel": "Тип нового файла",
  "tree.create.inVault": "Создать в vault",
  "tree.create.inFolder": (p) => `Создать в «${p.folder}»`,
  "tree.create.footCreate": "создать",
  "tree.create.footSelect": "↑↓ выбор",
  "tree.search.label": "Поиск по vault",
  "tree.search.placeholder": "поиск по vault…",
  "tree.navLabel": "Заметки",
  "tree.menu.createNote": "Создать заметку",
  "tree.menu.createBoard": "Создать доску",
  "tree.menu.fromTemplate": "Заметка из заготовки…",
  "tree.foldersCount": (p) => ruN(p.count, "# папка", "# папки", "# папок"),
  "tree.graph.title": "Граф связей",
  "tree.graph.open": "Открыть граф связей",

  "tree.open.opening": "Открываем…",
  "tree.open.continue": (p) => `Продолжить: ${p.label}`,
  "tree.open.chooseAnother": "Выбрать другую папку",
  "tree.open.open": "Открыть vault",

  "tree.defaultNoteName": "Новая заметка.typ",
  "tree.defaultBoardName": "Новая доска.excalidraw",
  "tree.confirm.delete": (p) =>
    `Удалить заметку «${p.name}»? Это действие нельзя отменить из интерфейса.`,
  "tree.confirm.deleteLinked": (p) =>
    `После удаления «${p.name}» останутся с битой ссылкой:\n${p.list}\n\nУдалить всё равно?`,
  "tree.confirm.renameLinked": (p) =>
    `После переименования «${p.from}» → «${p.to}» эти заметки продолжат ссылаться на старое имя, и их ссылки станут битыми:\n${p.list}\n\nПереименовать всё равно?`,
  "tree.menu.copyPath": "Копировать путь",
  "tree.menu.copied": (p) => `Скопировано: ${p.path}`,
  "tree.menu.open": "Открыть",
  "tree.menu.insertLink": "Вставить ссылку в открытую заметку",
  "tree.menu.deleteEllipsis": "Удалить…",
  "tree.menu.createNoteHere": "Создать заметку здесь",
  "tree.menu.createBoardHere": "Создать доску здесь",
  "tree.rename.label": (p) => `Новое имя для заметки ${p.name}`,
  "tree.unsaved": "есть несохранённые изменения",
  "tree.noteActions": (p) => `Действия с заметкой ${p.name}`,
  "tree.folderActions": (p) => `Действия с папкой ${p.name}`,
  "tree.rootFolder": "корень",
  "tree.section.mark": "Пометить как раздел",
  "tree.section.unmark": "Снять пометку раздела",
  "tree.section.rootTitle": "Корень раздела",
  "tree.section.badge": "раздел",
  "tree.create.nameLabel": "Имя новой заметки",
  "tree.create.nameOrBoardLabel": "Имя новой заметки или доски",
  "tree.emptyNotesOrBoards": "Vault пуст — создайте первую заметку или доску.",
};
