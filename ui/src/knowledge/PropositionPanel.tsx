"use client";

import { useEffect, useState } from "react";
import type { Chain, Proposition } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { failureKeyOf, type FailureKey, type OpenSource } from "./knowledgeText.js";
import { SourceLink } from "./SourceLink.js";
import { StatementButton } from "./StatementButton.js";

// Колонка выбранного утверждения (макет A, E): формулировки, «выводится из», где оно
// участвует, источники со ссылкой на место урока и удаление с предупреждением о цепочках,
// которые от него зависят.

export interface PropositionPanelProps {
  client: VaultClient;
  id: string;
  /** Меняется, когда граф пополнился: утверждение перечитывается. */
  revision: number;
  onSelect: (id: string) => void;
  onOpenSource: OpenSource;
  onClose: () => void;
  /** Утверждение удалено: вид перечитывает граф. */
  onRemoved: () => void;
}

type Loaded =
  | { state: "loading" }
  | { state: "gone" }
  | { state: "ready"; proposition: Proposition };

export function PropositionPanel({
  client,
  id,
  revision,
  onSelect,
  onOpenSource,
  onClose,
  onRemoved,
}: PropositionPanelProps): React.JSX.Element {
  const { t } = useI18n();
  const [loaded, setLoaded] = useState<Loaded>({ state: "loading" });
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<FailureKey | null>(null);

  useEffect(() => {
    let alive = true;
    setConfirming(false);
    setError(null);
    client.knowledge
      .proposition(id)
      .then((found) => alive && setLoaded(found === undefined ? { state: "gone" } : { state: "ready", proposition: found }))
      .catch(() => alive && setLoaded({ state: "gone" }));
    return () => {
      alive = false;
    };
  }, [client, id, revision]);

  const remove = async (): Promise<void> => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await client.knowledge.remove({ id });
      setConfirming(false);
      onRemoved();
    } catch (failure) {
      setError(failureKeyOf(failure));
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside className="ts-kg__side" aria-label={t("knowledge.panel.label")}>
      <div className="ts-kg__side-head">
        <span className="ts-kg__side-title">
          {loaded.state === "ready" && loaded.proposition.text}
          {loaded.state === "loading" && <span className="ts-kg__hint">{t("knowledge.panel.loading")}</span>}
          {loaded.state === "gone" && <span className="ts-kg__hint">{t("knowledge.panel.gone")}</span>}
        </span>
        <button
          type="button"
          className="ts-btn ts-btn--ghost"
          aria-label={t("knowledge.panel.close")}
          onClick={onClose}
        >
          <Icon name="x" size={14} />
        </button>
      </div>

      {loaded.state === "ready" && (
        <PanelBody
          proposition={loaded.proposition}
          confirming={confirming}
          busy={busy}
          error={error}
          onSelect={onSelect}
          onOpenSource={onOpenSource}
          onAskRemove={() => setConfirming(true)}
          onCancelRemove={() => setConfirming(false)}
          onRemove={() => void remove()}
        />
      )}
    </aside>
  );
}

function PanelBody({
  proposition,
  confirming,
  busy,
  error,
  onSelect,
  onOpenSource,
  onAskRemove,
  onCancelRemove,
  onRemove,
}: {
  proposition: Proposition;
  confirming: boolean;
  busy: boolean;
  error: FailureKey | null;
  onSelect: (id: string) => void;
  onOpenSource: OpenSource;
  onAskRemove: () => void;
  onCancelRemove: () => void;
  onRemove: () => void;
}): React.JSX.Element {
  const { t } = useI18n();
  const derived = proposition.chains.filter((chain) => chain.conclusion.id === proposition.id);
  const usedIn = proposition.chains.filter((chain) => chain.premises.some((premise) => premise.id === proposition.id));
  const others = proposition.phrasings.filter((phrasing) => phrasing !== proposition.text);

  return (
    <>
      {others.map((phrasing) => (
        <div key={phrasing} className="ts-kg__alt">
          {t("knowledge.panel.also", { text: phrasing })}
        </div>
      ))}

      <div className="ts-kg__lbl">{t("knowledge.panel.derived")}</div>
      {derived.length === 0 && <div className="ts-kg__hint">{t("knowledge.panel.noChains")}</div>}
      {derived.map((chain) => (
        <ChainBox key={chain.id} chain={chain} selfId={proposition.id} onSelect={onSelect} />
      ))}

      {usedIn.length > 0 && (
        <>
          <div className="ts-kg__lbl">{t("knowledge.panel.usedIn")}</div>
          {usedIn.map((chain) => (
            <ChainBox key={chain.id} chain={chain} selfId={proposition.id} onSelect={onSelect} showConclusion />
          ))}
        </>
      )}

      <div className="ts-kg__lbl">{t("knowledge.panel.sources")}</div>
      {proposition.sources.map((source) => (
        <div key={`${source.path}:${source.event ?? ""}`} className="ts-kg__srcrow">
          <SourceLink source={source} onOpen={onOpenSource} />
        </div>
      ))}

      {!confirming && (
        <div className="ts-kg__acts">
          <button type="button" className="ts-btn ts-btn--sm ts-btn--danger" onClick={onAskRemove}>
            {t("knowledge.remove.start")}
          </button>
        </div>
      )}
      {confirming && (
        <div className="ts-lesson__box ts-lesson__box--err ts-kg__confirm" role="alertdialog" aria-label={t("knowledge.remove.start")}>
          <p>{t("knowledge.remove.confirm", { text: proposition.text })}</p>
          {usedIn.length > 0 && <p>{t("knowledge.remove.incomplete", { count: usedIn.length })}</p>}
          {derived.length > 0 && <p>{t("knowledge.remove.hidden", { count: derived.length })}</p>}
          {error !== null && (
            <p className="ts-lesson__bad" role="alert">
              {t(error)}
            </p>
          )}
          <div className="ts-lesson__acts">
            <button type="button" className="ts-btn ts-btn--sm" autoFocus onClick={onCancelRemove}>
              {t("knowledge.remove.cancel")}
            </button>
            <button type="button" className="ts-btn ts-btn--sm ts-btn--danger" disabled={busy} onClick={onRemove}>
              {t("knowledge.remove.do")}
            </button>
          </div>
        </div>
      )}
    </>
  );
}

/** Цепочка в колонке: «посылка + посылка → вывод»; само выбранное утверждение — «это утверждение». */
function ChainBox({
  chain,
  selfId,
  onSelect,
  showConclusion = false,
}: {
  chain: Chain;
  selfId: string;
  onSelect: (id: string) => void;
  showConclusion?: boolean;
}): React.JSX.Element {
  const { t } = useI18n();
  const self = <span className="ts-kg__self">{t("knowledge.panel.self")}</span>;
  return (
    <div className="ts-kg__chain">
      {chain.premises.map((premise, index) => (
        <span key={premise.id} className="ts-kg__chain-part">
          {index > 0 && <span className="ts-kg__op"> + </span>}
          {premise.id === selfId ? self : <StatementButton statement={premise} selected={false} onSelect={onSelect} />}
        </span>
      ))}
      <span className="ts-kg__op"> → </span>
      {showConclusion ? <StatementButton statement={chain.conclusion} selected={false} onSelect={onSelect} /> : self}
      {chain.incomplete && <span className="ts-kg__badge">{t("knowledge.panel.incomplete")}</span>}
    </div>
  );
}
