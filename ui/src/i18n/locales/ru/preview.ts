import type { Translation } from "../define.js";
import type { preview as en } from "../en/preview.js";

export const preview: Translation<typeof en> = {
  "preview.label": "Предпросмотр",
  "preview.pageSize": "A4 · 21×29.7 см",
  "preview.building": "собирается…",
  "preview.stale": "не соответствует тексту",
  "preview.zoom.label": "Масштаб предпросмотра",
  "preview.zoom.out": "Уменьшить масштаб",
  "preview.zoom.outHint": "Уменьшить масштаб (Ctrl + колесо)",
  "preview.zoom.value": (p) => `Масштаб ${p.percent}%, вернуть 100%`,
  "preview.zoom.reset": "Вернуть 100 %",
  "preview.zoom.in": "Увеличить масштаб",
  "preview.zoom.inHint": "Увеличить масштаб (Ctrl + колесо)",
  "preview.zoom.inShort": "Увеличить",
  "preview.zoom.outShort": "Уменьшить",
  "preview.zoom.hundred": "Масштаб 100 %",
  "preview.failedShowingPrevious": "Заметка не собралась — показан прошлый вид",
  "preview.failedNothingToShow": "Показывать нечего: заметка ещё ни разу не собралась",
  "preview.buildFailed": (p) => `Сборка не состоялась: ${p.detail}`,
  "preview.openNote": "Откройте заметку, чтобы увидеть её свёрстанный вид.",
  "preview.buildingLong": "Собирается…",
  "preview.warnings": "Предупреждения",
  "preview.reveal": (p) => `Перейти к ${p.place}`,
  "preview.installing": "Загружается…",
  "preview.install": "Загрузить шаблон",
  "preview.page": (p) => `Страница ${p.number}`,
};
