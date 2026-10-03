import type { Translation } from "../define.js";
import { ruN } from "../define.js";
import type { graph as en } from "../en/graph.js";

export const graph: Translation<typeof en> = {
  "graph.title": "Граф связей",
  "graph.back": "Workspace",
  "graph.notes": (p) => ruN(p.count, "# заметка", "# заметки", "# заметок"),
  "graph.links": (p) => ruN(p.count, "# связь", "# связи", "# связей"),
  "graph.linksPending": "связи считаются…",
  "graph.filterLabel": "Фильтр по заметкам",
  "graph.filter": "фильтр по заметкам…",
  "graph.legendCurrent": "текущая",
  "graph.legendLinked": "связана",
  "graph.legendOrphan": "без связей",
  "graph.collecting": "Собираем связи…",
  "graph.empty": "В этом vault ещё нет заметок.",
  "graph.open": (p) => `Открыть ${p.name}`,
};
