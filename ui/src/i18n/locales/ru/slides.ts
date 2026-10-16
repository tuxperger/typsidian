import type { Translation } from "../define.js";
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
};
