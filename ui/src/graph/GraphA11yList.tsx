import { useMemo } from "react";
import { useI18n } from "../i18n/index.js";
import type { VaultGraph } from "../wasm-core/typsidian_core.js";

// Скрытый список узлов для чтения с экрана и обхода Tab (фича 025, T015,
// research.md §11). Порядок — по названию; на огромном графе берутся самые
// связанные узлы, чтобы не раздувать дерево документа.

const LIMIT = 1000;

export interface GraphA11yListProps {
  graph: VaultGraph;
  onFocusNode: (index: number | null) => void;
  onOpen: (index: number) => void;
}

export function GraphA11yList({ graph, onFocusNode, onOpen }: GraphA11yListProps): React.JSX.Element {
  const { t } = useI18n();
  const items = useMemo(() => {
    const indices = graph.nodes.map((_, index) => index);
    if (indices.length > LIMIT) {
      indices.sort((a, b) => graph.nodes[b]!.degree - graph.nodes[a]!.degree);
      indices.length = LIMIT;
    }
    return indices.sort((a, b) => graph.nodes[a]!.label.localeCompare(graph.nodes[b]!.label));
  }, [graph]);

  return (
    <ul className="ts-graph__sr" aria-label={t("graph.list.label")}>
      {items.map((index) => {
        const node = graph.nodes[index]!;
        return (
          <li key={node.id}>
            <button
              type="button"
              onFocus={() => onFocusNode(index)}
              onBlur={() => onFocusNode(null)}
              onClick={() => onOpen(index)}
            >
              {t("graph.list.item", {
                name: node.label,
                kind: t(`graph.kind.${node.kind}`),
                links: t("graph.links", { count: node.degree }),
              })}
            </button>
          </li>
        );
      })}
    </ul>
  );
}
