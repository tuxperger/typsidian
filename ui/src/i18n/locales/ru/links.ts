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
};
