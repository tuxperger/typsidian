"use client";

import type { Chain } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import type { OpenSource } from "./knowledgeText.js";
import { SourceLink } from "./SourceLink.js";
import { StatementButton } from "./StatementButton.js";

// Таблица цепочек (макет B): то же содержимое, что у карты, для узкого экрана и для
// выключенной карты (FR-007). Посылки и вывод открывают колонку утверждения.

export interface KnowledgeChainsProps {
  chains: readonly Chain[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onOpenSource: OpenSource;
}

export function KnowledgeChains({ chains, selectedId, onSelect, onOpenSource }: KnowledgeChainsProps): React.JSX.Element {
  const { t } = useI18n();
  if (chains.length === 0) {
    return (
      <p className="ts-kg__empty" role="status">
        {t("knowledge.chains.empty")}
      </p>
    );
  }
  return (
    <div className="ts-kg__tablewrap">
      <table className="ts-kg__table" aria-label={t("knowledge.chains.label")}>
        <thead>
          <tr>
            <th scope="col">{t("knowledge.chains.premises")}</th>
            <th scope="col" aria-hidden="true" />
            <th scope="col">{t("knowledge.chains.conclusion")}</th>
            <th scope="col">{t("knowledge.chains.source")}</th>
          </tr>
        </thead>
        <tbody>
          {chains.map((chain) => (
            <tr key={chain.id} data-incomplete={chain.incomplete}>
              <td data-label={t("knowledge.chains.premises")}>
                {chain.premises.map((premise) => (
                  <div key={premise.id}>
                    <StatementButton statement={premise} selected={premise.id === selectedId} onSelect={onSelect} />
                  </div>
                ))}
                {chain.incomplete && <span className="ts-kg__badge">{t("knowledge.panel.incomplete")}</span>}
              </td>
              <td className="ts-kg__arrow" aria-hidden="true">
                →
              </td>
              <td data-label={t("knowledge.chains.conclusion")}>
                <StatementButton
                  statement={chain.conclusion}
                  selected={chain.conclusion.id === selectedId}
                  onSelect={onSelect}
                />
              </td>
              <td className="ts-kg__srccell" data-label={t("knowledge.chains.source")}>
                {chain.sources.map((source) => (
                  <div key={`${source.path}:${source.event ?? ""}`}>
                    <SourceLink source={source} onOpen={onOpenSource} compact />
                  </div>
                ))}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
