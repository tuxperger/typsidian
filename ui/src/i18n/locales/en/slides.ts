import type { Entries } from "../define.js";
import { enN } from "../define.js";

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

  // ── Export (feature 035) ───────────────────────────────────────────
  "slides.export.menu": "Export presentation…",
  "slides.export.command": "Export presentation",
  "slides.export.title": "Export presentation",
  "slides.export.close": "Close",
  "slides.export.format": "Format",
  "slides.export.pdf": "Slides PDF",
  "slides.export.pdfHint": "a page per slide",
  "slides.export.notes": "PDF with notes pages",
  "slides.export.notesHint": "the slide and the speaker notes under it",
  "slides.export.pdfpc": "PDF for pdfpc",
  "slides.export.pdfpcHint": "all steps and a .pdfpc notes file next to it",
  "slides.export.png": "PNG images",
  "slides.export.pngHint": "a file per slide in a .slides folder",
  "slides.export.svg": "SVG images",
  "slides.export.svgHint": "vector, a file per slide",
  "slides.export.steps": "Steps",
  "slides.export.lastStep": "last step",
  "slides.export.allSteps": "all steps",
  "slides.export.scale": "Scale",
  "slides.export.start": "Export",
  "slides.export.running": "Exporting…",
  "slides.export.done": (p: { count: number }) => `Done: ${enN(p.count, "# page", "# pages")}`,
  "slides.export.more": (p: { count: number }) => `and ${p.count} more`,
  "slides.export.failed": "The presentation didn’t build — nothing was written.",
  "slides.export.noSlides": "The presentation has no pages.",
  "slides.export.writeFailed": "Couldn’t write the files to the export folder.",
  "slides.export.folder": "Files go to the export folder of the vault",
} as const satisfies Entries;
