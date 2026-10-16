import type { Entries } from "../define.js";

// Режим показа слайдов (`slides.*`) — фича 019.
export const slides = {
  // ── Запуск (A1) ────────────────────────────────────────────────────
  "slides.button": "Show",
  "slides.menu.label": "Show options",
  "slides.menu.fromCurrent": "From the current slide",
  "slides.menu.fromCurrentHint": "The slide under the caret",
  "slides.menu.fromStart": "From the start",
  "slides.menu.withPresenter": "Straight to presenter view",
  "slides.menu.withPresenterHint": "A second window opens",
  "slides.command.fromCurrent": "Show slides: from the current slide",
  "slides.command.fromStart": "Show slides: from the start",
  "slides.command.withPresenter": "Show slides: with presenter view",
  "slides.blocked":
    "The show window did not open. The browser may have blocked pop-up windows for this site.",
  "slides.reopen": "Open again",
  "slides.dismiss": "Dismiss",

  // ── Окно показа (A2–A6) ────────────────────────────────────────────
  "slides.windowTitle": "Typsidian — slide show",
  "slides.view.label": "Slide show",
  "slides.view.loading": "Loading the show…",
  "slides.slideAlt": (p: { current: number; total: number }) =>
    `Slide ${p.current} of ${p.total}`,
  "slides.previous": "Previous slide",
  "slides.next": "Next slide",
  "slides.counter": (p: { current: number; total: number }) => `${p.current} / ${p.total}`,
  "slides.step": (p: { step: number; steps: number }) => `step ${p.step} of ${p.steps}`,
  "slides.presenter": "Presenter",
  "slides.presenterHint": "Open or close the presenter view",
  "slides.fullscreen": "Fullscreen",
  "slides.exitFullscreen": "Leave fullscreen",
  "slides.exit": "Exit the show",
  "slides.exitKey": "Esc",
  "slides.toolbar": "Show controls",
  "slides.goto.label": "Go to slide",
  "slides.goto.hint": (p: { total: number }) => `Enter — go · Esc — cancel · of ${p.total}`,
  "slides.stale": "Showing the last successful build",
  "slides.staleAt": (p: { line: number }) => `Showing the last successful build · error at line ${p.line}`,
  "slides.toError": "To the error",
  "slides.empty.title": "Nothing to show",
  "slides.empty.body": "The note does not build, and there has been no successful build yet.",
  "slides.empty.back": "Back to the note",
  "slides.empty.close": "Close the show",
  "slides.fullscreenHint":
    "The browser did not expand the window. Press F11 or use the Fullscreen button below.",

  // ── Вид докладчика (A7) ────────────────────────────────────────────
  "slides.presenter.label": "Presenter view",
  "slides.presenter.clock": "Now",
  "slides.presenter.elapsed": "Elapsed",
  "slides.presenter.onScreen": "On screen",
  "slides.presenter.upNext": "Next",
  "slides.presenter.notes": "Speaker notes",
  "slides.presenter.end": "End the show",
  "slides.presenter.strip": "Slides",
  "slides.presenter.goToPage": (p: { page: number }) => `Go to page ${p.page}`,

  // ── Android (A8) ───────────────────────────────────────────────────
  "slides.overlay.exit": "Exit",
} as const satisfies Entries;
