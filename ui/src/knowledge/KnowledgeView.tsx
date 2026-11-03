"use client";

import { useEffect, useMemo, useState } from "react";
import type { Chain, KnowledgeGraph } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { usePhoneViewport } from "../workspace/usePhoneViewport.js";
import { KnowledgeChains } from "./KnowledgeChains.js";
import { KnowledgeJobs } from "./KnowledgeJobs.js";
import { KnowledgeMap } from "./KnowledgeMap.js";
import type { OpenSource } from "./knowledgeText.js";
import { PropositionPanel } from "./PropositionPanel.js";
import { useKnowledgeJobs } from "./useKnowledgeJobs.js";

// Вид «Граф знаний» на месте текста (фича 054, макет A, B, F): переключатель «Карта /
// Цепочки», поиск, счётчики, колонка выбранного утверждения и полоса задания. Что в графе,
// как он ищется и в какой форме отдаётся, решает ядро (`ai::knowledge`); здесь — показ.

const SEARCH_DEBOUNCE_MS = 200;

type Mode = "map" | "chains";

export interface KnowledgeViewProps {
  client: VaultClient | null;
  /** Id записи vault на устройстве — по нему опрашиваются задания. */
  vaultId: string;
  /** Открыть источник: урок на месте события. */
  onOpenSource: OpenSource;
  /** «Назад» — только на телефоне: на рабочем столе выход через шапку. */
  onBack?: (() => void) | undefined;
}

export function KnowledgeView({ client, vaultId, onOpenSource, onBack }: KnowledgeViewProps): React.JSX.Element {
  const { t } = useI18n();
  const phone = usePhoneViewport();
  const [mapEnabled, setMapEnabled] = useState(true);
  const [picked, setPicked] = useState<Mode | null>(null);
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const [graph, setGraph] = useState<KnowledgeGraph | null>(null);
  const [chains, setChains] = useState<Chain[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  const { jobs } = useKnowledgeJobs(client, vaultId);

  // «Показывать картой» выключено — остаётся только таблица цепочек (FR-007).
  useEffect(() => {
    if (client === null) return;
    let alive = true;
    client.ai
      .settings()
      .then((settings) => alive && setMapEnabled(settings.knowledge_map ?? true))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [client]);

  const mode: Mode = !mapEnabled ? "chains" : (picked ?? (phone ? "chains" : "map"));

  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(input.trim()), SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [input]);

  // Задание подвигало граф: пачка записана или задание кончилось.
  const progress = jobs.reduce(
    (sum, job) => sum + job.batches_done + (job.state === "Running" ? 0 : 1000),
    0,
  );
  const revision = reload + progress;

  useEffect(() => {
    if (client === null) return;
    let alive = true;
    client.knowledge
      .graph(query === "" ? undefined : query)
      .then((loaded) => {
        if (!alive) return;
        setGraph(loaded);
        setFailed(false);
      })
      .catch(() => alive && setFailed(true));
    return () => {
      alive = false;
    };
  }, [client, query, revision]);

  useEffect(() => {
    if (client === null || mode !== "chains") return;
    let alive = true;
    client.knowledge
      .chains(query === "" ? undefined : query)
      .then((loaded) => alive && setChains(loaded))
      .catch(() => alive && setFailed(true));
    return () => {
      alive = false;
    };
  }, [client, mode, query, revision]);

  const counts = useMemo(
    () =>
      graph === null
        ? ""
        : t("knowledge.count", {
            statements: t("knowledge.statements", { count: graph.propositions }),
            chains: t("knowledge.chainsCount", { count: graph.chains }),
            sources: t("knowledge.sourcesCount", { count: graph.sources }),
          }),
    [graph, t],
  );

  if (client === null) return <section className="ts-kg" aria-label={t("knowledge.title")} />;

  const nothing = graph !== null && graph.propositions === 0 && query === "";
  const noMatches = graph !== null && graph.propositions === 0 && query !== "";


  return (
    <section className="ts-kg" aria-label={t("knowledge.title")}>
      <div className="ts-kg__bar">
        {onBack !== undefined && (
          <button type="button" className="ts-btn" onClick={onBack}>
            <Icon name="chevron" size={13} className="ts-icon--flip" />
            {t("knowledge.back")}
          </button>
        )}
        <b className="ts-kg__name">{t("knowledge.title")}</b>
        {mapEnabled && (
          <div className="ts-graph__seg" role="group" aria-label={t("knowledge.mode.label")}>
            <button type="button" aria-pressed={mode === "map"} onClick={() => setPicked("map")}>
              {t("knowledge.mode.map")}
            </button>
            <button type="button" aria-pressed={mode === "chains"} onClick={() => setPicked("chains")}>
              {t("knowledge.mode.chains")}
            </button>
          </div>
        )}
        <input
          className="ts-field__input ts-kg__search"
          type="search"
          aria-label={t("knowledge.search")}
          placeholder={t("knowledge.search")}
          value={input}
          onChange={(event) => setInput(event.target.value)}
        />
        <span className="ts-spacer" />
        {!nothing && (
          <span className="ts-kg__count" aria-live="polite">
            {counts}
          </span>
        )}
      </div>

      {graph?.damaged === true && (
        <div className="ts-lesson__ro" role="status">
          {t("knowledge.damaged")}
        </div>
      )}

      <div className="ts-kg__split" data-side={selectedId !== null} data-empty={nothing}>
        <div className="ts-kg__main">
          {graph === null && (
            <p className="ts-kg__empty" role="status">
              {failed ? t("knowledge.loadFailed") : t("knowledge.loading")}
            </p>
          )}
          {nothing && (
            <p className="ts-kg__empty" role="status">
              {t("knowledge.empty")}
            </p>
          )}
          {noMatches && (
            <p className="ts-kg__empty" role="status">
              {t("knowledge.emptySearch")}
            </p>
          )}
          {graph !== null && graph.propositions > 0 && mode === "map" && (
            <KnowledgeMap graph={graph} selectedId={selectedId} onSelect={setSelectedId} />
          )}
          {graph !== null && graph.propositions > 0 && mode === "chains" && chains !== null && (
            <KnowledgeChains
              chains={chains}
              selectedId={selectedId}
              onSelect={setSelectedId}
              onOpenSource={onOpenSource}
            />
          )}
        </div>
        {selectedId !== null ? (
          <PropositionPanel
            client={client}
            id={selectedId}
            revision={revision}
            onSelect={setSelectedId}
            onOpenSource={onOpenSource}
            onClose={() => setSelectedId(null)}
            onRemoved={() => {
              setSelectedId(null);
              setReload((n) => n + 1);
            }}
          />
        ) : nothing ? null : (
          <aside className="ts-kg__side ts-kg__side--idle" aria-label={t("knowledge.panel.label")}>
            <p className="ts-kg__hint">{t("knowledge.panel.empty")}</p>
          </aside>
        )}
      </div>

      <KnowledgeJobs client={client} vaultId={vaultId} jobs={jobs} />
    </section>
  );
}
