import type { Entries } from "../define.js";
import { enN } from "../define.js";

export const graph = {
  "graph.title": "Link graph",
  "graph.back": "Workspace",
  "graph.notes": (p: { count: number }) => enN(p.count, "# note", "# notes"),
  "graph.links": (p: { count: number }) => enN(p.count, "# link", "# links"),
  "graph.linksPending": "counting links…",
  "graph.filterLabel": "Filter notes",
  "graph.filter": "filter notes…",
  "graph.legendCurrent": "current",
  "graph.legendLinked": "linked",
  "graph.legendOrphan": "no links",
  "graph.collecting": "Collecting links…",
  "graph.empty": "This vault has no notes yet.",
  "graph.open": (p: { name: string }) => `Open ${p.name}`,
} as const satisfies Entries;
