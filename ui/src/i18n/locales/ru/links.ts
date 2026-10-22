import type { Translation } from "../define.js";
import { ruN } from "../define.js";
import type { links as en } from "../en/links.js";

// Колонка заметки (`notePanel.*`) и её режим «Связи» (`links.*`) — фича 013.
export const links: Translation<typeof en> = {
  // ── Оболочка колонки ───────────────────────────────────────────────
  "notePanel.button": "Панель заметки",
  "notePanel.command": "Показать или скрыть панель заметки",
  "notePanel.outlineCommand": "Показать или скрыть оглавление заметки",
  "notePanel.label": "Панель заметки",
  "notePanel.modes": "Режим панели",
  "notePanel.close": "Закрыть панель заметки",
  "notePanel.openNote": "Откройте заметку, чтобы увидеть её связи.",

  // ── Режим «Связи» ──────────────────────────────────────────────────
  "links.mode": "Связи",
  "links.backlinks.title": "Обратные ссылки",
  "links.mentions.title": "Упоминания",
  "links.backlinks.collecting": "Собираю сведения о ссылках vault… Список появится сам.",
  "links.backlinks.failed": "Не удалось загрузить ссылки. Список обновится при следующем изменении.",
  "links.backlinks.empty": "На эту заметку пока никто не ссылается.",
  "links.backlinks.emptyExample": "Ссылка из другой заметки —",
  "links.problem.tooLarge": "Заметка больше предела размера — фрагмент не показан.",
  "links.problem.unreadable": "Заметка не читается — фрагмент не показан.",
  "links.count": (p) => ruN(p.count, "# ссылка", "# ссылки", "# ссылок"),
  "links.notesCount": (p) => ruN(p.count, "# заметка", "# заметки", "# заметок"),
  "links.rowLabel": (p) => `открыть ${p.note}, строка ${p.line}`,
  "links.partial": (p) =>
    `показано ${p.shown} из ${p.total} заметок, ближайшие по папке — первыми`,
  "links.more": (p) => `Показать ещё ${p.count}`,

  // ── Упоминания ─────────────────────────────────────────────────────
  "links.mentions.collapsed": "раскройте, чтобы найти",
  "links.mentions.scanning": (p) => `Ищу упоминания: ${p.scanned} из ${p.total} заметок`,
  "links.mentions.collapse": "Свернуть",
  "links.mentions.tooShort": (p) =>
    `Название «${p.title}» короче трёх знаков — упоминания не ищутся: совпадений было бы слишком много.`,
  "links.mentions.none": "Название нигде не упомянуто без ссылки.",
  "links.mentions.failed": "Не удалось найти упоминания.",
  "links.mentions.link": "Связать",
  "links.mentions.linkLabel": (p) => `связать упоминание в ${p.note}, строка ${p.line}`,
  "links.mentions.readOnly": "Vault открыт только на чтение — связать нельзя.",
  "links.mentions.stale": "устарело",
  "links.mentions.staleNote":
    "Упоминания на этом месте уже нет — заметка изменилась. Список обновлён, файл не тронут.",
  "links.mentions.unsaved": "В заметке есть несохранённые правки — ссылка не записана.",
  "links.mentions.unexpressible": "Ссылку на эту заметку записать нельзя.",
  "links.mentions.writeFailed": "Не удалось записать ссылку.",
  "links.mentions.noEditor":
    "Редактор открытой заметки не принимает эту правку — файл не тронут.",

  // ── Строка состояния редактора ─────────────────────────────────────
  "links.status.links": (p) => ruN(p.count, "# ссылка", "# ссылки", "# ссылок"),
  "links.status.mentions": (p) => ruN(p.count, "# упоминание", "# упоминания", "# упоминаний"),

  // ── Состояние ссылок (фича 028) ────────────────────────────────────
  "linkHealth.title": "Состояние ссылок",
  "linkHealth.command": "Состояние ссылок: битые ссылки и заметки без входящих",
  "linkHealth.filterBroken": "фильтр по цели или заметке…",
  "linkHealth.filterOrphans": "фильтр по имени или папке…",
  "linkHealth.filterLabel": "Фильтр",
  "linkHealth.hideEntries": "скрыть входы",
  "linkHealth.hideEntriesHint": "Входящие и дневные заметки без ссылок по природе",
  "linkHealth.loading": "собираю…",
  "linkHealth.brokenCount": (p) =>
    `${ruN(p.links, "# битая ссылка", "# битые ссылки", "# битых ссылок")} · ${ruN(p.targets, "# цель", "# цели", "# целей")}`,
  "linkHealth.close": "Закрыть состояние ссылок",
  "linkHealth.tabBroken": "Битые ссылки",
  "linkHealth.tabOrphans": "Без входящих",
  "linkHealth.noBroken": "Все ссылки ведут на существующие заметки.",
  "linkHealth.noOrphans": "На каждую заметку кто-то ссылается.",
  "linkHealth.entry": "вход",
  "linkHealth.open": "Открыть",
  "linkHealth.intentional": "Намеренно",
  "linkHealth.intentionalHint": "Больше не показывать эту заметку здесь в этом vault",
  "linkHealth.hintBroken": "Щелчок по строке — открыть заметку на месте ссылки · Esc закрыть",
  "linkHealth.hintOrphans": "«Намеренно» — больше не показывать в этом vault",
  "linkHealth.hiddenIntentional": (p) => `скрыто намеренно: ${p.count}`,
  "linkHealth.kind.note": "заметка",
  "linkHealth.kind.board": "доска",
  "linkHealth.kind.attachment": "вложение",
  "linkHealth.embeds": (p) => ruN(p.count, "# вставка", "# вставки", "# вставок"),
  "linkHealth.createNote": "Создать заметку",
  "linkHealth.createBoard": "Создать доску",
  "linkHealth.redirect": "Перенаправить на…",
  "linkHealth.unlinkAll": "Убрать ссылки",
  "linkHealth.removeEmbeds": "Убрать вставку",
  "linkHealth.line": (p) => `стр. ${p.line}`,
  "linkHealth.unlinkOne": "Убрать эту ссылку, оставив текст",
  "linkHealth.moreSites": (p) => `и ещё ${p.count}`,
  "linkHealth.pickPlaceholder": "название заметки…",
  "linkHealth.pickLabel": "Заметка, на которую перенаправить",
  "linkHealth.pickNothing": "Такой заметки нет.",
  "linkHealth.readOnly": "Vault только для чтения — ничего не изменено.",
  "linkHealth.created": (p) => `Создано «${p.name}»`,
  "linkHealth.fixedNotes": (p) =>
    `Исправлено в ${ruN(p.count, "# заметке", "# заметках", "# заметках")}, часть пропущена`,
  "linkHealth.redirected": (p) =>
    `Перенаправлено ${ruN(p.count, "# ссылка", "# ссылки", "# ссылок")} на «${p.name}»`,
  "linkHealth.unlinked": (p) => `Убрано ${ruN(p.count, "# ссылка", "# ссылки", "# ссылок")}`,
  "linkHealth.undo": "Отменить",
  "linkHealth.dismiss": "Скрыть итог",
  "linkHealth.skipped": (p) =>
    `Пропущено — заметка изменилась или есть несохранённые правки: ${p.names}`,
  "linkHealth.footer": (p) => ruN(p.count, "# битая", "# битые", "# битых"),
  "linkHealth.footerTitle": "Открыть состояние ссылок",
};
