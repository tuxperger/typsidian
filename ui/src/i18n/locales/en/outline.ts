import type { Entries } from "../define.js";

// Оглавление заметки (фича 015): режим «Структура» колонки заметки, боковая
// шторка узкого окна. Макет — `Typsidian Оглавление.dc.html`, экраны 1a–1c.
export const outline = {
  // ── Режим и команда ────────────────────────────────────────────────
  "outline.mode": "Outline",
  "outline.button": "Outline",
  "outline.treeLabel": "Note outline",
  "outline.loading": "Reading the headings…",

  // ── Фильтр ─────────────────────────────────────────────────────────
  "outline.filter.label": "Filter the outline",
  "outline.filter.placeholder": "filter…",

  // ── Пустые состояния (FR-012) ──────────────────────────────────────
  "outline.empty.openNote": "Open a note to see its outline.",
  "outline.empty.noHeadings": "This note has no headings. Start a line with = and they will appear here.",
  "outline.empty.noMatches": "Nothing found",
  "outline.empty.board": "Boards have no outline",
  "outline.empty.tooLarge": "The note is too large — no outline is built",

  // ── Строки ─────────────────────────────────────────────────────────
  "outline.prefix.figure": "fig.",
  "outline.prefix.table": "tab.",
  "outline.row.expand": "Expand the section",
  "outline.row.collapse": "Collapse the section",

  // ── Виды пунктов (FR-010) ──────────────────────────────────────────
  "outline.kinds.menu": "Show in the outline",
  "outline.kinds.figures": "Figures and tables",
  "outline.kinds.labels": "Labels",

  // ── Шторка узкого окна (макет 1b) ──────────────────────────────────
  "outline.drawer.esc": "esc",
  "outline.drawer.close": "Close the panel",
  "outline.drawer.footer": "↑↓ navigate · ↵ jump · esc close",
} as const satisfies Entries;
