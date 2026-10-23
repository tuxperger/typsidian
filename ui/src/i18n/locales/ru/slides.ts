import type { Translation } from "../define.js";
import { ruN } from "../define.js";
import type { slides as en } from "../en/slides.js";

// Режим показа слайдов (`slides.*`) — фича 019.
export const slides: Translation<typeof en> = {
  // ── Запуск (A1) ────────────────────────────────────────────────────
  "slides.button": "Показ",
  "slides.menu.label": "Варианты показа",
  "slides.menu.fromCurrent": "С текущего слайда",
  "slides.menu.fromCurrentHint": "Слайд под кареткой",
  "slides.menu.fromStart": "С начала",
  "slides.menu.withPresenter": "Сразу с видом докладчика",
  "slides.menu.withPresenterHint": "Откроется второе окно",
  "slides.command.fromCurrent": "Показ слайдов: с текущего слайда",
  "slides.command.fromStart": "Показ слайдов: с начала",
  "slides.command.withPresenter": "Показ слайдов: с видом докладчика",
  "slides.blocked":
    "Окно показа не открылось. Возможно, браузер заблокировал всплывающие окна для этого сайта.",
  "slides.reopen": "Открыть ещё раз",
  "slides.dismiss": "Закрыть",

  // ── Окно показа (A2–A6) ────────────────────────────────────────────
  "slides.windowTitle": "Typsidian — показ слайдов",
  "slides.view.label": "Показ слайдов",
  "slides.view.loading": "Загружаю показ…",
  "slides.slideAlt": (p) => `Слайд ${p.current} из ${p.total}`,
  "slides.previous": "Предыдущий слайд",
  "slides.next": "Следующий слайд",
  "slides.counter": (p) => `${p.current} / ${p.total}`,
  "slides.step": (p) => `шаг ${p.step} из ${p.steps}`,
  "slides.presenter": "Докладчик",
  "slides.presenterHint": "Открыть или закрыть вид докладчика",
  "slides.fullscreen": "Во весь экран",
  "slides.exitFullscreen": "Выйти из полного экрана",
  "slides.exit": "Выйти из показа",
  "slides.exitKey": "Esc",
  "slides.toolbar": "Управление показом",
  "slides.goto.label": "Переход к слайду",
  "slides.goto.hint": (p) => `Enter — перейти · Esc — отмена · из ${p.total}`,
  "slides.stale": "Показана последняя удачная сборка",
  "slides.staleAt": (p) => `Показана последняя удачная сборка · ошибка в строке ${p.line}`,
  "slides.toError": "К ошибке",
  "slides.empty.title": "Показывать нечего",
  "slides.empty.body": "Заметка не собирается, а удачной сборки ещё не было.",
  "slides.empty.back": "Вернуться к заметке",
  "slides.empty.close": "Закрыть показ",
  "slides.fullscreenHint":
    "Браузер не дал развернуть окно. Нажмите F11 или кнопку «Во весь экран» ниже.",

  // ── Вид докладчика (A7) ────────────────────────────────────────────
  "slides.presenter.label": "Вид докладчика",
  "slides.presenter.clock": "Сейчас",
  "slides.presenter.elapsed": "С начала",
  "slides.presenter.onScreen": "Сейчас на экране",
  "slides.presenter.upNext": "Дальше",
  "slides.presenter.notes": "Заметки докладчика",
  "slides.presenter.end": "Завершить показ",
  "slides.presenter.strip": "Слайды",
  "slides.presenter.goToPage": (p) => `Перейти к странице ${p.page}`,

  // ── Android (A8) ───────────────────────────────────────────────────
  "slides.overlay.exit": "Выйти",

  // ── Экспорт (фича 035) ─────────────────────────────────────────────
  "slides.export.menu": "Экспорт презентации…",
  "slides.export.command": "Экспорт презентации",
  "slides.export.title": "Экспорт презентации",
  "slides.export.close": "Закрыть",
  "slides.export.format": "Формат",
  "slides.export.pdf": "PDF слайдов",
  "slides.export.pdfHint": "страница на слайд",
  "slides.export.notes": "PDF со страницами заметок",
  "slides.export.notesHint": "слайд и заметки докладчика под ним",
  "slides.export.pdfpc": "PDF для pdfpc",
  "slides.export.pdfpcHint": "все шаги и файл заметок .pdfpc рядом",
  "slides.export.png": "Картинки PNG",
  "slides.export.pngHint": "файл на слайд в папке .slides",
  "slides.export.svg": "Картинки SVG",
  "slides.export.svgHint": "векторные, файл на слайд",
  "slides.export.steps": "Шаги",
  "slides.export.lastStep": "последний шаг",
  "slides.export.allSteps": "все шаги",
  "slides.export.scale": "Масштаб",
  "slides.export.start": "Экспортировать",
  "slides.export.running": "Экспортирую…",
  "slides.export.done": (p) => `Готово: ${ruN(p.count, "# страница", "# страницы", "# страниц")}`,
  "slides.export.more": (p) => `и ещё ${p.count}`,
  "slides.export.failed": "Презентация не собралась — ничего не записано.",
  "slides.export.noSlides": "В презентации нет страниц.",
  "slides.export.writeFailed": "Не удалось записать файлы в папку экспорта.",
  "slides.export.folder": "Файлы — в папке экспорта vault",
};
