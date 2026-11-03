import type { Translation } from "../define.js";
import { ruN } from "../define.js";
import type { knowledge as en } from "../en/knowledge.js";

// Граф знаний из уроков (`knowledge.*`) — фича 054, макет A–F.
export const knowledge: Translation<typeof en> = {
  "knowledge.command": "Граф знаний",
  "knowledge.title": "Граф знаний",
  "knowledge.open": "Граф знаний",
  "knowledge.loading": "Загружаем граф знаний…",
  "knowledge.loadFailed": "Не удалось загрузить граф знаний.",
  "knowledge.back": "Назад",

  "knowledge.mode.label": "Вид графа знаний",
  "knowledge.mode.map": "Карта",
  "knowledge.mode.chains": "Цепочки",
  "knowledge.search": "Найти утверждение…",
  "knowledge.statements": (p) => ruN(p.count, "# утверждение", "# утверждения", "# утверждений"),
  "knowledge.chainsCount": (p) => ruN(p.count, "# цепочка", "# цепочки", "# цепочек"),
  "knowledge.sourcesCount": (p) => ruN(p.count, "# источник", "# источника", "# источников"),
  "knowledge.count": (p) => `${p.statements} · ${p.chains} · ${p.sources}`,
  "knowledge.canvasLabel": (p) => `Карта знаний: ${p.statements}, ${p.chains}`,

  "knowledge.empty": "Граф пополняется из уроков: учитель объясняет — здесь появляются утверждения и связи между ними.",
  "knowledge.emptySearch": "Ничего не найдено. Попробуйте другие слова.",
  "knowledge.damaged": "Файлы графа знаний читаются не целиком. Показано то, что удалось прочитать; пока файлы не исправлены, запись в них отключена.",

  "knowledge.legend.title": "Обозначения",
  "knowledge.legend.statement": "утверждение",
  "knowledge.legend.fork": "развилка цепочки",
  "knowledge.legend.selected": "выбранное",
  "knowledge.zoomIn": "Приблизить",
  "knowledge.zoomOut": "Отдалить",
  "knowledge.zoomFit": "Вписать в экран",

  "knowledge.panel.label": "Утверждение",
  "knowledge.panel.empty": "Выберите утверждение на карте или в таблице: здесь появятся его формулировки, цепочки и источники.",
  "knowledge.panel.loading": "Загружаем утверждение…",
  "knowledge.panel.gone": "Этого утверждения в графе больше нет.",
  "knowledge.panel.close": "Закрыть",
  "knowledge.panel.also": (p) => `Также: «${p.text}»`,
  "knowledge.panel.derived": "Выводится из",
  "knowledge.panel.usedIn": "Участвует в выводе",
  "knowledge.panel.sources": "Источники",
  "knowledge.panel.self": "это утверждение",
  "knowledge.panel.incomplete": "неполная",
  "knowledge.panel.noChains": "Пока ни из чего не выводится.",

  "knowledge.source.event": (p) => `реплика ${p.n}`,
  "knowledge.source.removed": "источник удалён",
  "knowledge.source.open": (p) => `Открыть ${p.name}`,

  "knowledge.chains.label": "Цепочки рассуждений",
  "knowledge.chains.premises": "Посылки",
  "knowledge.chains.conclusion": "Вывод",
  "knowledge.chains.source": "Источник",
  "knowledge.chains.empty": "Цепочек пока нет: учитель ещё не выводил одно из другого.",

  "knowledge.remove.start": "Удалить утверждение…",
  "knowledge.remove.confirm": (p) => `Удалить утверждение «${p.text}»?`,
  "knowledge.remove.incomplete": (p) =>
    ruN(
      p.count,
      "# цепочка, где оно посылка, станет неполной.",
      "# цепочки, где оно посылка, станут неполными.",
      "# цепочек, где оно посылка, станут неполными.",
    ),
  "knowledge.remove.hidden": (p) =>
    ruN(
      p.count,
      "# цепочка, где оно вывод, будет скрыта.",
      "# цепочки, где оно вывод, будут скрыты.",
      "# цепочек, где оно вывод, будут скрыты.",
    ),
  "knowledge.remove.cancel": "Отмена",
  "knowledge.remove.do": "Удалить",

  "knowledge.job.label": "Пополнение графа знаний",
  "knowledge.job.running": (p) => `Пополняем граф из «${p.name}»`,
  "knowledge.job.batches": (p) => `часть ${p.done} из ${p.total}`,
  "knowledge.job.starting": "готовим",
  "knowledge.job.cancel": "Отменить",
  "knowledge.job.failed": (p) => `Не удалось пополнить граф из «${p.name}».`,
  "knowledge.job.failedKept": "Уже добавленное остаётся в графе.",
  "knowledge.job.dismiss": "Скрыть",

  "knowledge.lesson.summary": (p) => `В граф добавлено ${p.statements} и ${p.chains}`,
  "knowledge.lesson.summaryMerged": (p) =>
    `${ruN(p.count, "# утверждение уже было", "# утверждения уже были", "# утверждений уже было")} в графе и объединено.`,
  "knowledge.lesson.summaryNone": "Нового в граф не добавлено.",
  "knowledge.lesson.open": "открыть",
  "knowledge.lesson.fill": "Пополнить граф",
  "knowledge.lesson.fillHint": (p) => `Ещё не в графе: около ${p.count} символов объяснений.`,
  "knowledge.lesson.failed": "Не удалось пополнить граф.",

  "knowledge.settings.title": "Граф знаний",
  "knowledge.settings.auto": "Пополнять граф после шагов урока",
  "knowledge.settings.autoHint": "В граф уходят только объяснения учителя, не ваши ответы.",
  "knowledge.settings.agentReads": "Агент может сам читать граф",
  "knowledge.settings.agentReadsHint": "Иначе граф уходит модели только по вашему выбору.",
  "knowledge.settings.map": "Показывать картой",
  "knowledge.settings.mapHint": "Выключено — только таблица цепочек.",

  "knowledge.read.sent": (p) =>
    `Агент прочитал граф знаний по запросу «${p.query}» — отправлено ${ruN(p.count, "# утверждение", "# утверждения", "# утверждений")}`,
  "knowledge.read.denied": (p) => `Агент запросил граф знаний по запросу «${p.query}» — папка закрыта для агента`,
  "knowledge.material": (p) =>
    `Опора на граф знаний: ${ruN(p.count, "# утверждение", "# утверждения", "# утверждений")}`,

  "knowledge.pick.label": "Опора из графа знаний",
  "knowledge.pick.add": "Добавить…",
  "knowledge.pick.search": "Поиск по графу",
  "knowledge.pick.none": "Граф пока пуст: утверждения появятся после уроков.",
  "knowledge.pick.noMatches": "Утверждений не найдено.",
  "knowledge.pick.remove": (p) => `Убрать «${p.text}»`,
  "knowledge.pick.limit": (p) => `Не больше ${p.max} утверждений.`,
  "knowledge.pick.hint": "Текст выбранных утверждений уйдёт модели и будет виден в сессии и в журнале.",
  "knowledge.pick.failed": "Не удалось загрузить граф.",

  "knowledge.delete.action": "Удалить…",
  "knowledge.delete.aria": (p) => `Удалить урок «${p.goal}»`,
  "knowledge.delete.title": (p) => `Удалить урок «${p.goal}»?`,
  "knowledge.delete.keep": "Оставить его вклад в графе",
  "knowledge.delete.drop": (p) => `Удалить и вклад: ${p.statements}, ${p.chains}`,
  "knowledge.delete.dropNone": "Удалить и вклад",
  "knowledge.delete.cancel": "Отмена",
  "knowledge.delete.do": "Удалить урок",
  "knowledge.delete.noGraph": "Файл урока будет удалён. Вклада в граф знаний у него нет.",

  "knowledge.failure.JournalFailed": "Не удалось записать в файлы графа знаний или в журнал отправленного: возможно, они повреждены. Ничего не изменено.",
  "knowledge.failure.Forbidden": "Урок или папка графа закрыты для агента в настройках.",
  "knowledge.failure.LessonUnreadable": "Урок или утверждение не найдены, либо файл не читается.",
  "knowledge.failure.Cancelled": "Граф из этого урока уже пополняется.",
};
