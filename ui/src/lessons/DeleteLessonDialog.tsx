"use client";

import { useEffect, useId, useState } from "react";
import type { LessonSummary } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";
import { failureKeyOf, type FailureKey } from "../knowledge/knowledgeText.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

// Удаление урока с выбором судьбы его вклада в граф знаний (фича 054, макет E): оставить
// утверждения (источник помечается удалённым) либо удалить их вместе с уроком. Числа во
// втором варианте — приблизительные: утверждения, у которых этот урок первый источник, и
// цепочки, где он единственный. Точное удаление вклада делает ядро.

export interface DeleteLessonDialogProps {
  client: VaultClient;
  lesson: LessonSummary;
  onDeleted: () => void;
  onCancel: () => void;
}

interface Contribution {
  statements: number;
  chains: number;
}

export function DeleteLessonDialog({ client, lesson, onDeleted, onCancel }: DeleteLessonDialogProps): React.JSX.Element {
  const { t } = useI18n();
  const id = useId();
  useDismissOnEscape(onCancel);
  const [contribution, setContribution] = useState<Contribution | null>(null);
  const [keep, setKeep] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<FailureKey | null>(null);

  useEffect(() => {
    let alive = true;
    void Promise.all([client.knowledge.graph(), client.knowledge.chains()])
      .then(([graph, chains]) => {
        if (!alive) return;
        const statements = graph.groups.find((group) => group.source === lesson.path)?.count ?? 0;
        const own = chains.filter(
          (chain) => chain.sources.length > 0 && chain.sources.every((source) => source.path === lesson.path),
        ).length;
        setContribution({ statements, chains: own });
      })
      // Граф не прочитан (повреждён, нет папки): вклад неизвестен, урок удаляется с его сохранением.
      .catch(() => alive && setContribution({ statements: 0, chains: 0 }));
    return () => {
      alive = false;
    };
  }, [client, lesson.path]);

  const has = contribution !== null && (contribution.statements > 0 || contribution.chains > 0);

  const remove = async (): Promise<void> => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      // Без вклада выбирать нечего: источник просто помечается удалённым.
      await client.lessons.delete(lesson.path, has ? keep : true);
      onDeleted();
    } catch (failure) {
      setError(failureKeyOf(failure));
      setBusy(false);
    }
  };

  return (
    <div className="ts-overlay ts-overlay--center" role="presentation">
      <section
        className="ts-lesson-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={`${id}-title`}
      >
        <h2 className="ts-lesson-dialog__title" id={`${id}-title`}>
          {t("knowledge.delete.title", { goal: lesson.goal })}
        </h2>
        {contribution !== null && !has && <p className="ts-lesson__hint">{t("knowledge.delete.noGraph")}</p>}
        {has && (
          <div className="ts-lessons__delbox" role="radiogroup" aria-labelledby={`${id}-title`}>
            <label>
              <input type="radio" name={`${id}-keep`} checked={keep} onChange={() => setKeep(true)} />
              {t("knowledge.delete.keep")}
            </label>
            <label>
              <input type="radio" name={`${id}-keep`} checked={!keep} onChange={() => setKeep(false)} />
              {t("knowledge.delete.drop", {
                statements: t("knowledge.statements", { count: contribution.statements }),
                chains: t("knowledge.chainsCount", { count: contribution.chains }),
              })}
            </label>
          </div>
        )}
        {error !== null && (
          <p className="ts-lesson__bad" role="alert">
            {t(error)}
          </p>
        )}
        <div className="ts-lesson__acts">
          <button type="button" className="ts-btn" autoFocus onClick={onCancel}>
            {t("knowledge.delete.cancel")}
          </button>
          <button
            type="button"
            className="ts-btn ts-btn--danger"
            disabled={busy || contribution === null}
            onClick={() => void remove()}
          >
            {t("knowledge.delete.do")}
          </button>
        </div>
      </section>
    </div>
  );
}
