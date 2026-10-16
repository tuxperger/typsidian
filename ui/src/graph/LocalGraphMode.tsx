"use client";

import { useCallback, useMemo, useState } from "react";
import { useI18n } from "../i18n/index.js";
import type { VaultClient } from "../vault-client.js";
import type { GraphNode } from "../wasm-core/typsidian_core.js";
import { GraphA11yList } from "./GraphA11yList.js";
import { GraphCanvas } from "./GraphCanvas.js";
import { GraphDepth } from "./GraphDepth.js";
import { emptyReason, GraphEmpty, openablePath } from "./GraphView.js";
import { useGraphLayout } from "./useGraphLayout.js";
import type { ResolvedGraphSettings } from "./useGraphSettings.js";
import { buildRequest, useVaultGraph } from "./useVaultGraph.js";

// Режим колонки заметки «Граф» (фича 025, T019, макет G3, research.md §10):
// локальный граф вокруг открытой заметки, подписи всегда, без панели.

export interface LocalGraphModeProps {
  client: VaultClient | null;
  /** Открытая заметка; `null` — ничего не открыто. */
  currentPath: string | null;
  settings: ResolvedGraphSettings;
  depth: number;
  onDepthChange: (depth: number) => void;
  onOpenPath: (path: string) => void;
  onOpenAll: () => void;
}

export function LocalGraphMode({
  client,
  currentPath,
  settings,
  depth,
  onDepthChange,
  onOpenPath,
  onOpenAll,
}: LocalGraphModeProps): React.JSX.Element {
  const { t } = useI18n();
  const [focused, setFocused] = useState<number | null>(null);
  const request = useMemo(
    () => buildRequest(settings, currentPath === null ? null : { center: currentPath, depth }),
    [settings, currentPath, depth],
  );
  const { loaded } = useVaultGraph(client, request);
  const graph = loaded?.graph ?? null;
  const layout = useGraphLayout(graph, settings.forces, loaded?.renames ?? []);

  const open = useCallback(
    (node: GraphNode) => {
      const path = openablePath(node);
      if (path !== null) onOpenPath(path);
    },
    [onOpenPath],
  );

  const empty = graph === null ? null : emptyReason(graph, "local", currentPath, "");
  const nodesText = graph === null ? "" : t("graph.nodes", { count: graph.nodes.length });

  return (
    <div className="ts-graph-mode">
      <div className="ts-graph-mode__canvas">
        {graph !== null && empty === null && (
          <>
            <GraphCanvas
              graph={graph}
              layout={layout}
              groups={settings.groups}
              arrows={settings.arrows}
              labelZoom={settings.label_zoom}
              nodeSize={settings.node_size}
              currentPath={currentPath}
              alwaysLabels
              fitKey={`${currentPath ?? ""}:${depth}`}
              focusedIndex={focused}
              ariaLabel={t("graph.canvasLabel", {
                nodes: nodesText,
                links: t("graph.links", { count: graph.edges.length }),
              })}
              onOpen={open}
            />
            <GraphA11yList
              graph={graph}
              onFocusNode={setFocused}
              onOpen={(index) => {
                const node = graph.nodes[index];
                if (node !== undefined) open(node);
              }}
            />
          </>
        )}
        {empty !== null && <GraphEmpty reason={empty} name={currentPath} />}
      </div>
      <GraphDepth depth={depth} onChange={onDepthChange} />
      <p className="ts-graph-mode__hint">
        {t("graph.local.hint")} ·{" "}
        <button type="button" className="ts-graph__link" onClick={onOpenAll}>
          {t("graph.local.openAll")}
        </button>
      </p>
    </div>
  );
}
