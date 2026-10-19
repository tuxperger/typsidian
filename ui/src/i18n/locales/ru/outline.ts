import type { Translation } from "../define.js";
import type { outline as en } from "../en/outline.js";

// Оглавление заметки (фича 015): режим «Структура» колонки заметки, боковая
// шторка узкого окна. Макет — `Typsidian Оглавление.dc.html`, экраны 1a–1c.
export const outline: Translation<typeof en> = {
  // ── Режим и команда ────────────────────────────────────────────────
  "outline.mode": "Структура",
  "outline.button": "Оглавление",
  "outline.treeLabel": "Оглавление заметки",
  "outline.loading": "Читаю заголовки…",

  // ── Фильтр ─────────────────────────────────────────────────────────
  "outline.filter.label": "Фильтр оглавления",
  "outline.filter.placeholder": "фильтр…",

  // ── Пустые состояния (FR-012) ──────────────────────────────────────
  "outline.empty.openNote": "Откройте заметку, чтобы увидеть её оглавление.",
  "outline.empty.noHeadings": "В заметке нет заголовков. Начните строку с =, и они появятся здесь.",
  "outline.empty.noMatches": "Ничего не найдено",
  "outline.empty.board": "У доски нет оглавления",
  "outline.empty.tooLarge": "Заметка слишком велика — оглавление не строится",

  // ── Строки ─────────────────────────────────────────────────────────
  "outline.prefix.figure": "рис.",
  "outline.prefix.table": "табл.",
  "outline.row.expand": "Развернуть раздел",
  "outline.row.collapse": "Свернуть раздел",

  // ── Виды пунктов (FR-010) ──────────────────────────────────────────
  "outline.kinds.menu": "Показывать в оглавлении",
  "outline.kinds.figures": "Рисунки и таблицы",
  "outline.kinds.labels": "Метки",

  // ── Шторка узкого окна (макет 1b) ──────────────────────────────────
  "outline.drawer.esc": "esc",
  "outline.drawer.close": "Закрыть панель",
  "outline.drawer.footer": "↑↓ навигация · ↵ перейти · esc закрыть",
};
