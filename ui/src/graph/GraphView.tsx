"use client";

import { useCallback, useMemo, useRef, useState } from "react";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { basename } from "../paths.js";
import type { VaultClient } from "../vault-client.js";
import type { GraphNode, VaultGraph } from "../wasm-core/typsidian_core.js";
import { GraphA11yList } from "./GraphA11yList.js";
import { GraphCanvas, type GraphCanvasHandle } from "./GraphCanvas.js";
import { GraphDepth } from "./GraphDepth.js";
import { GraphPanel } from "./GraphPanel.js";
import { useGraphLayout } from "./useGraphLayout.js";
import type { GraphSettingsState } from "./useGraphSettings.js";
import { buildRequest, useVaultGraph } from "./useVaultGraph.js";

// Экран графа связей (фича 025, макет G1, G2, G4, G5): верхняя полоса с охватом
// и глубиной, холст, панель настроек справа (на узком экране — шторка снизу).
// Граф строит ядро одним вызовом; здесь он только рисуется (принцип I).

export type GraphScope = "vault" | "local";

export interface GraphViewProps {
  client: VaultClient | null;
  /** Открытая заметка — центр локального графа и кольцо на холсте. */
  currentPath: string | null;
  settings: GraphSettingsState;
  scope: GraphScope;
  onScopeChange: (scope: GraphScope) => void;
  depth: number;
  onDepthChange: (depth: number) => void;
  onOpenPath: (path: string) => void;
  /**
   * «Назад» на полотне — только на телефоне: на рабочем столе граф стоит
   * внутри рабочей области, и выход — вид в шапке или значок дерева (044).
   */
  onBack?: (() => void) | undefined;
}

/** Узлы, которые открываются щелчком: заметки и доски. */
export function openablePath(node: GraphNode): string | null {
  if (node.kind === "Note" || node.kind === "Board") return node.path ?? null;
  return null;
}

export function GraphView({
  client,
  currentPath,
  settings: settingsState,
  scope,
  onScopeChange,
  depth,
  onDepthChange,
  onOpenPath,
  onBack,
}: GraphViewProps): React.JSX.Element {
  const { t } = useI18n();
  const { settings } = settingsState;
  const canvasRef = useRef<GraphCanvasHandle | null>(null);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [focused, setFocused] = useState<number | null>(null);

  const local = scope === "local" && currentPath !== null;
  const request = useMemo(
    () => buildRequest(settings, local ? { center: currentPath, depth } : null),
    [settings, local, currentPath, depth],
  );
  const { loaded, loading, failed } = useVaultGraph(client, request);
  const graph = loaded?.graph ?? null;
  const layout = useGraphLayout(graph, settings.forces, loaded?.renames ?? []);

  const open = useCallback(
    (node: GraphNode) => {
      const path = openablePath(node);
      if (path !== null) onOpenPath(path);
    },
    [onOpenPath],
  );

  const empty = graph === null ? null : emptyReason(graph, scope, currentPath, settings.filter);
  const nodesText = (count: number): string => t("graph.nodes", { count });
  const countText =
    graph === null
      ? ""
      : local && graph.nodes.length > 0
        ? t("graph.countLocal", {
            name: basename(currentPath).replace(/\.[^.]+$/, ""),
            nodes: nodesText(graph.nodes.length),
          })
        : t("graph.count", {
            nodes: nodesText(graph.nodes.length),
            links: t("graph.links", { count: graph.edges.length }),
          });

  return (
    <section className="ts-graph" aria-label={t("graph.title")}>
      <div className="ts-graph__stage">
        {/* Колонка «Показывать» / «Обозначения» — слева от полотна (фича 044,
            артборд 7); на узком экране — шторка снизу, как прежде. */}
        <div className="ts-graph__side" data-open={sheetOpen}>
          <GraphPanel state={settingsState} />
        </div>
        <div className="ts-graph__canvas">
          {graph !== null && empty === null && (
            <>
              <GraphCanvas
                ref={canvasRef}
                graph={graph}
                layout={layout}
                groups={settings.groups}
                arrows={settings.arrows}
                labelZoom={settings.label_zoom}
                nodeSize={settings.node_size}
                currentPath={currentPath}
                alwaysLabels={false}
                fitKey={`${scope}:${local ? currentPath : ""}`}
                focusedIndex={focused}
                ariaLabel={t("graph.canvasLabel", {
                  nodes: nodesText(graph.nodes.length),
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
          {graph === null && (
            <p className="ts-graph__empty" role="status">
              {failed ? t("graph.saveFailed") : t("graph.loading")}
            </p>
          )}

          <div className="ts-graph__top">
            {onBack !== undefined && (
              <button type="button" className="ts-btn" onClick={onBack}>
                <Icon name="chevron" size={13} className="ts-icon--flip" />
                {t("graph.back")}
              </button>
            )}
            <div className="ts-graph__seg" role="group" aria-label={t("graph.mode.label")}>
              <button
                type="button"
                aria-pressed={scope === "vault"}
                onClick={() => onScopeChange("vault")}
              >
                {t("graph.mode.vault")}
              </button>
              <button
                type="button"
                aria-pressed={scope === "local"}
                onClick={() => onScopeChange("local")}
              >
                {t("graph.mode.local")}
              </button>
            </div>
            {scope === "local" && <GraphDepth depth={depth} onChange={onDepthChange} />}
          </div>

          {graph !== null && empty === null && (
            <div className="ts-graph__zoom">
              <button
                type="button"
                aria-label={t("graph.zoomIn")}
                onClick={() => canvasRef.current?.zoomBy(1.4)}
              >
                +
              </button>
              <button
                type="button"
                aria-label={t("graph.zoomOut")}
                onClick={() => canvasRef.current?.zoomBy(1 / 1.4)}
              >
                −
              </button>
              <button
                type="button"
                aria-label={t("graph.zoomFit")}
                onClick={() => canvasRef.current?.fit()}
              >
                ⤢
              </button>
            </div>
          )}
          <button
            type="button"
            className="ts-btn ts-graph__sheet-toggle"
            aria-expanded={sheetOpen}
            onClick={() => setSheetOpen((value) => !value)}
          >
            {sheetOpen ? t("graph.settings.close") : t("graph.settings.open")}
          </button>
        </div>
      </div>
      {/* Сводка графа — в строке состояния под ним (артборд 7). */}
      <div className="ts-statusbar ts-graph__status">
        <span className="ts-graph__count" aria-live="polite" data-loading={loading}>
          {countText}
        </span>
      </div>
    </section>
  );
}

export type EmptyReason =
  | "vault"
  | "vaultNoLinks"
  | "filtered"
  | "noCurrent"
  | "noLinks";

/** Почему вместо графа пояснение (макет G4); `null` — граф есть. */
export function emptyReason(
  graph: VaultGraph,
  scope: GraphScope,
  currentPath: string | null,
  filter: string,
): EmptyReason | null {
  if (scope === "local") {
    if (currentPath === null) return "noCurrent";
    return graph.nodes.length <= 1 ? "noLinks" : null;
  }
  if (graph.nodes.length > 0) return null;
  if (graph.total_notes === 0) return "vault";
  if (filter.trim() !== "") return "filtered";
  return "vaultNoLinks";
}

export function GraphEmpty({
  reason,
  name,
}: {
  reason: EmptyReason;
  name: string | null;
}): React.JSX.Element {
  const { t } = useI18n();
  const title = name === null ? "" : basename(name).replace(/\.[^.]+$/, "");
  return (
    <div className="ts-graph__empty" role="status">
      <div>
        {reason === "vault" && <b>{t("graph.empty.vault")}</b>}
        {reason === "filtered" && <b>{t("graph.empty.filtered")}</b>}
        {reason === "vaultNoLinks" && (
          <>
            <b>{t("graph.empty.vaultNoLinksTitle")}</b>
            <span>{t("graph.empty.vaultNoLinksHint")}</span>
          </>
        )}
        {reason === "noCurrent" && (
          <>
            <b>{t("graph.empty.noCurrentTitle")}</b>
            <span>{t("graph.empty.noCurrentHint")}</span>
          </>
        )}
        {reason === "noLinks" && (
          <>
            <b>{t("graph.empty.noLinksTitle", { name: title })}</b>
            <span>{t("graph.empty.noLinksHint")}</span>
          </>
        )}
      </div>
    </div>
  );
}
