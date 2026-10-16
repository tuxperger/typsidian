import { ruN, type Translation } from "../define.js";
import type { capture as en } from "../en/capture.js";

// Быстрый захват и ежедневные заметки (`capture.*`) — фича 022.
export const capture: Translation<typeof en> = {
  // ── Лист захвата (D1–D3) ───────────────────────────────────────────
  "capture.sheet.label": "Быстрый захват",
  "capture.sheet.title": "В Typsidian",
  "capture.sheet.close": "Закрыть",
  "capture.manual.label": "Что записать",
  "capture.manual.placeholder": "Мысль, ссылка, заметка…",
  "capture.image.alt": (p) => `Предпросмотр: ${p.name}`,
  "capture.image.caption": (p) => `${p.name} · ${p.size}`,
  "capture.dest.label": "Куда записать",
  "capture.dest.inbox": "Входящие",
  "capture.dest.daily": "Сегодня",
  "capture.dest.recent": "Недавние",
  "capture.dest.recentChoose": "Недавние…",
  "capture.dest.recentNone": "Нет открытых заметок",
  "capture.note.label": "Приписка",
  "capture.note.placeholder": "Приписка…",
  "capture.save": "Сохранить",
  "capture.saving": "Сохраняю…",
  "capture.hint.append": (p) => `Допишется в конец «${p.path}» · ${p.time}`,
  "capture.hint.image": (p) => `Картинка — во вложения, вставка — в «${p.path}»`,
  "capture.hint.unknown": "Заметка назначения появится, когда станет известна",
  "capture.queue.more": (p) =>
    `В очереди ещё ${p.count} ${ruN(p.count, "захват", "захвата", "захватов")}`,

  // ── Отказ (D3) ─────────────────────────────────────────────────────
  "capture.failed.title": "Захват не записан",
  "capture.failed.noVault.title": "Нет доступа к папке vault",
  "capture.failed.noVault.body":
    "Захват сохранён и ждёт: откройте Typsidian и выберите папку заново.",
  "capture.failed.pending": (p) =>
    `Ждут записи: ${p.count} ${ruN(p.count, "захват", "захвата", "захватов")}`,
  "capture.failed.open": "Открыть Typsidian",
  "capture.failed.retry": "Повторить",
  "capture.failed.discard": "Отказаться от захвата",

  // ── Команды ────────────────────────────────────────────────────────
  "capture.command.inbox": "Во входящие",
  "capture.command.today": "Сегодня",
  "capture.command.previousDay": "Предыдущий день",
  "capture.command.nextDay": "Следующий день",

  // ── «Сегодня» и ‹ › (D4) ───────────────────────────────────────────
  "capture.daily.today": "Сегодня",
  "capture.daily.todayHint": "Открыть ежедневную заметку на сегодня",
  "capture.daily.previous": "Предыдущая ежедневная заметка",
  "capture.daily.next": "Следующая ежедневная заметка",
  "capture.daily.nav": "Ежедневные заметки",
  "capture.daily.noPrevious": "Раньше ежедневных заметок нет",
  "capture.daily.noNext": "Позже ежедневных заметок нет",
  "capture.daily.templateMissing":
    "Шаблон ежедневной заметки не найден: заметка создана только с заголовком",
  "capture.daily.failed": (p) => `Не удалось открыть заметку на сегодня: ${p.reason}`,
  "capture.daily.error.InvalidDate": "дата не годится",
  "capture.daily.error.ReadOnly": "vault только для чтения",
  "capture.daily.error.NoVault": "vault не открыт",
  "capture.daily.error.Io": "диск отказал в записи",

  // ── Настройки vault (D6) ───────────────────────────────────────────
  "capture.settings.title": "Захват и дневник",
  "capture.settings.openVault": "Откройте vault, чтобы настроить захват.",
  "capture.settings.inbox": "Заметка входящих",
  "capture.settings.inboxHint": "Создаётся при первом захвате, если её нет",
  "capture.settings.dailyFolder": "Папка ежедневных заметок",
  "capture.settings.dailyTemplate": "Шаблон ежедневной заметки",
  "capture.settings.dailyTemplateHint":
    "Из заготовок; без шаблона у новой заметки только заголовок с названием файла",
  "capture.settings.noTemplate": "Без шаблона",
  "capture.settings.dailyName": "Название по дате",
  "capture.settings.dailyNameToday": (p) => `Сегодня: ${p.path}`,
  "capture.settings.name.YYYY-MM-DD": "ГГГГ-ММ-ДД",
  "capture.settings.name.YYYY/MM/YYYY-MM-DD": "ГГГГ/ММ/ГГГГ-ММ-ДД",
  "capture.settings.name.DD.MM.YYYY": "ДД.ММ.ГГГГ",
  "capture.settings.entryStyle": "Вид записи захвата",
  "capture.settings.style.list": "Пункт списка",
  "capture.settings.style.paragraph": "Абзац",
  "capture.settings.style.time": "С временем",
  "capture.settings.sample": "Образец записи",
  "capture.settings.sampleTitle": "Как устроен планировщик PostgreSQL",
  "capture.settings.sampleUrl": "https://habr.com/ru/articles/851234",
  "capture.settings.sampleNote": "почитать к докладу",
  "capture.settings.footer": "Хранится вместе с vault: одинаково на всех устройствах",
  "capture.settings.invalid": (p) => `«${p.field}» не годится: ${p.problem}`,
};
