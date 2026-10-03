import type { Translation } from "../define.js";
import { ruN } from "../define.js";
import type { templates as en } from "../en/templates.js";

export const templates: Translation<typeof en> = {
  "templates.gallery.title": "Шаблоны и сниппеты",
  "templates.gallery.filter": "фильтр…",
  "templates.gallery.filterLabel": "Фильтр по имени заготовки",
  "templates.gallery.fromCurrent": "Из текущей заметки",
  "templates.gallery.close": "Закрыть галерею",
  "templates.gallery.list": "Заготовки",
  "templates.gallery.loading": "Читаем библиотеку…",
  "templates.gallery.noMatch": (p) => `По запросу «${p.filter}» заготовок нет.`,
  "templates.gallery.selected": "Выбранная заготовка",
  "templates.gallery.pickHint":
    "Выберите заготовку — её можно развернуть в новую заметку или вставить в текущую позицию курсора.",
  "templates.gallery.apply": "применить",
  "templates.gallery.createNote": "Создать заметку по шаблону",
  "templates.gallery.insertDisabled": "Откройте заметку, чтобы вставить заготовку в курсор",
  "templates.gallery.insert": "Вставить в курсор",
  "templates.gallery.openSource": "Открыть исходник шаблона",
  "templates.gallery.sideHintBefore": "Заготовка — обычная заметка в папке ",
  "templates.gallery.sideHintAfter": ": она видна в дереве, участвует в поиске и в графе ссылок.",
  "templates.gallery.count": (p) => ruN(p.count, "# заготовка", "# заготовки", "# заготовок"),
  "templates.gallery.doubleClick": "двойное нажатие — применить",
  "templates.gallery.escClose": "Esc закрыть",
  "templates.gallery.noEditor": "Вставлять некуда: заметка не открыта.",
  "templates.gallery.insertFailed": (p) => `Вставить не удалось: ${p.reason}.`,
  "templates.folder.unreadableBefore": "Папку ",
  "templates.folder.unreadableAfter": (p) =>
    ` не удалось прочитать: ${p.reason}. Остальное приложение работает как обычно.`,
  "templates.folder.readOnlyBefore": "Папка ",
  "templates.folder.readOnlyAfter": (p) =>
    ` доступна только на чтение: ${p.reason}. Заготовки можно применять, но сохранить новую нельзя.`,
  "templates.empty.unreadable": "Пока папка не читается, показать заготовки нечем.",
  "templates.empty.before": "Заготовок пока нет. Папка ",
  "templates.empty.after":
    " — обычная папка vault: положите в неё заметку любым способом, и она появится здесь. Папка создастся сама при первом сохранении заготовки из приложения.",
  "templates.folderProblem.accessDenied": "нет прав доступа",
  "templates.folderProblem.notADirectory": "по этому пути лежит файл, а не папка",
  "templates.folderProblem.vaultReadOnly": "весь vault открыт только на чтение",
  "templates.folderProblem.io": "отказ чтения с диска",
  "templates.card.rendered": "отрисованный вид",
  "templates.card.pending": "превью строится…",
  "templates.card.compilerUnavailable": "исходный текст: отрисованный вид пока не строится",
  "templates.card.doesNotCompile": "исходный текст: содержимое не собирается отдельным документом",
  "templates.card.notFound": "файла нет — его удалили или переименовали снаружи",
  "templates.card.unreadable": "файл не читается как текст",
  "templates.card.io": "файл не удалось прочитать с диска",
  "templates.read.notFound":
    "файл заготовки не найден — возможно, его удалили или переименовали снаружи",
  "templates.read.unreadable": "файл заготовки не читается как текст",
  "templates.read.io": "не удалось прочитать файл с диска",
  "templates.menu.fromTemplate": "Из заготовки",
  "templates.menu.blank": "Пустая заметка",
  "templates.menu.more": (p) => `Ещё ${p.count} — открыть галерею`,
  "templates.save.title": "Сохранить как заготовку",
  "templates.save.name": "Имя заготовки",
  "templates.save.hintBefore": "Файл появится в папке ",
  "templates.save.hintAfter": " — обычной папкой vault. Если её ещё нет, она создастся сама.",
  "templates.save.taken":
    "Заготовка с таким именем уже есть. Перезапись заменит её содержимое — отменить это из приложения нельзя.",
  "templates.save.overwrite": "Перезаписать",
  "templates.save.empty": "Имя не может быть пустым.",
  "templates.save.illegal":
    "В имени есть символы, недопустимые для файловой системы: разделители пути «/» и «\\».",
  "templates.save.nameTaken": "Заготовка с таким именем уже есть — подтвердите перезапись.",
  "templates.save.folderNotWritable": (p) =>
    `Папку templates/ нельзя изменить: ${p.reason}. Исходная заметка не тронута.`,
  "templates.save.io": "Записать файл не удалось. Исходная заметка не тронута.",
};
