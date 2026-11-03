"use client";

import { useMemo, useRef, useState } from "react";
import type { GraphGroup, GraphNode, KnowledgeGraph } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import { GraphA11yList } from "../graph/GraphA11yList.js";
import { GraphCanvas, type GraphCanvasHandle } from "../graph/GraphCanvas.js";
import { GRAPH_DEFAULTS } from "../graph/useGraphSettings.js";
import { useGraphLayout } from "../graph/useGraphLayout.js";
import { sourceName } from "./knowledgeText.js";

// Карта знаний (макет A): холст и раскладка графа заметок 025 без правок. Ядро отдаёт узлы в
// форме `VaultGraph`: утверждение — `Note` с id `p_…`, развилка цепочки из нескольких
// посылок — `Tag` с пустой подписью; индекс группы узла — источник-урок.

/** Развилки подписей не имеют, поэтому от выбора они выпадают. */
const NO_RENAMES: [string, string][] = [];
/** Групп в легенде карты, остальные сворачиваются в счётчик. */
const LEGEND_MAX = 8;

export interface KnowledgeMapProps {
  graph: KnowledgeGraph;
  selectedId: string | null;
  onSelect: (id: string) => void;
}

export function KnowledgeMap({ graph, selectedId, onSelect }: KnowledgeMapProps): React.JSX.Element {
  const { t } = useI18n();
  const canvasRef = useRef<GraphCanvasHandle | null>(null);
  const [focused, setFocused] = useState<number | null>(null);
  const layout = useGraphLayout(graph.graph, GRAPH_DEFAULTS.forces, NO_RENAMES);
  // Индексы групп узлов — индексы в `graph.groups`: запрос группы здесь только метка.
  const groups = useMemo<GraphGroup[]>(
    () => graph.groups.map((group) => ({ query: group.source, color: group.color })),
    [graph.groups],
  );
  const statements = t("knowledge.statements", { count: graph.propositions });
  const chains = t("knowledge.chainsCount", { count: graph.chains });

  const open = (node: GraphNode): void => {
    // Развилка — не утверждение: открывать нечего.
    if (node.kind === "Note") onSelect(node.id);
  };
  const legend = graph.groups.filter((group) => group.count > 0);

  return (
    <div className="ts-graph__canvas ts-kg__canvas">
      <GraphCanvas
        ref={canvasRef}
        graph={graph.graph}
        layout={layout}
        groups={groups}
        arrows
        labelZoom={GRAPH_DEFAULTS.label_zoom}
        nodeSize={GRAPH_DEFAULTS.node_size}
        currentPath={selectedId}
        alwaysLabels={false}
        fitKey={String(graph.graph.nodes.length)}
        focusedIndex={focused}
        ariaLabel={t("knowledge.canvasLabel", { statements, chains })}
        onOpen={open}
      />
      <GraphA11yList
        graph={graph.graph}
        onFocusNode={setFocused}
        onOpen={(index) => {
          const node = graph.graph.nodes[index];
          if (node !== undefined) open(node);
        }}
      />
      <div className="ts-graph__zoom">
        <button type="button" aria-label={t("knowledge.zoomIn")} onClick={() => canvasRef.current?.zoomBy(1.4)}>
          +
        </button>
        <button type="button" aria-label={t("knowledge.zoomOut")} onClick={() => canvasRef.current?.zoomBy(1 / 1.4)}>
          −
        </button>
        <button type="button" aria-label={t("knowledge.zoomFit")} onClick={() => canvasRef.current?.fit()}>
          ⤢
        </button>
      </div>
      <ul className="ts-kg__legend" aria-label={t("knowledge.legend.title")}>
        {legend.slice(0, LEGEND_MAX).map((group) => (
          <li key={group.source}>
            <span className="ts-kg__dot" data-color={group.color} aria-hidden="true" />
            <span className="ts-kg__legend-name">{group.label !== "" ? group.label : sourceName(group.source)}</span>
            <span className="ts-kg__hint">{group.count}</span>
          </li>
        ))}
        {legend.length > LEGEND_MAX && <li className="ts-kg__hint">+{legend.length - LEGEND_MAX}</li>}
        <li>
          <span className="ts-kg__dot ts-kg__dot--fork" aria-hidden="true" />
          <span className="ts-kg__legend-name">{t("knowledge.legend.fork")}</span>
        </li>
      </ul>
    </div>
  );
}
