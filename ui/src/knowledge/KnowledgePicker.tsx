"use client";

import { useEffect, useId, useState } from "react";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";
import { useClosedPaths } from "../lessons/aiReadiness.js";
import { PICK_MAX } from "./knowledgeText.js";

// Выбор утверждений графа знаний как опоры нового урока или практики (фича 054, макет A
// диалогов, FR-009): поиск по графу, чипы выбранного, не больше 50. Текст выбранного уйдёт
// модели и будет виден в сессии и в журнале — об этом подсказка под полем. Если папка графа
// закрыта для агента, выбор недоступен: ядро всё равно отклонило бы его (`Forbidden`).

const SEARCH_DEBOUNCE_MS = 200;
const RESULTS_SHOWN = 8;
const CHIP_CHARS = 56;

export interface PickedStatement {
  id: string;
  text: string;
}

export interface KnowledgePickerProps {
  client: VaultClient;
  /** Папка уроков из настроек: от неё считается папка графа. */
  lessonsFolder: string;
  chosen: readonly PickedStatement[];
  onChange: (next: PickedStatement[]) => void;
}

/** Файл графа, по которому проверяется доступ агента (research 054 §1); пусто — папка неизвестна. */
export function knowledgeGraphPath(lessonsFolder: string): string {
  const folder = lessonsFolder.replace(/^\/+|\/+$/g, "");
  return folder === "" ? "" : `${folder}/Граф знаний/propositions.jsonl`; // i18n-ignore
}

function clip(text: string): string {
  const chars = Array.from(text);
  return chars.length > CHIP_CHARS ? `${chars.slice(0, CHIP_CHARS - 1).join("")}…` : text;
}

export function KnowledgePicker({ client, lessonsFolder, chosen, onChange }: KnowledgePickerProps): React.JSX.Element {
  const { t } = useI18n();
  const id = useId();
  const [total, setTotal] = useState<number | null>(null);
  const [picking, setPicking] = useState(false);
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const [found, setFound] = useState<PickedStatement[] | null>(null);
  const [failed, setFailed] = useState(false);
  const graphPath = knowledgeGraphPath(lessonsFolder);
  const shut = useClosedPaths(client, graphPath === "" ? [] : [graphPath]).has(graphPath);

  // Сколько утверждений в графе: пустой граф выбирать не из чего.
  useEffect(() => {
    let alive = true;
    client.knowledge
      .graph()
      .then((graph) => alive && setTotal(graph.propositions))
      .catch(() => alive && setFailed(true));
    return () => {
      alive = false;
    };
  }, [client]);

  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(input.trim()), SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [input]);

  useEffect(() => {
    if (!picking) return;
    let alive = true;
    client.knowledge
      .graph(query === "" ? undefined : query)
      .then((graph) => {
        if (!alive) return;
        setFailed(false);
        setFound(
          graph.graph.nodes
            .filter((node) => node.kind === "Note")
            .map((node) => ({ id: node.id, text: node.label })),
        );
      })
      .catch(() => alive && setFailed(true));
    return () => {
      alive = false;
    };
  }, [client, picking, query]);

  const taken = new Set(chosen.map((item) => item.id));
  const results = (found ?? []).filter((item) => !taken.has(item.id)).slice(0, RESULTS_SHOWN);
  const full = chosen.length >= PICK_MAX;

  return (
    <div className="ts-kg__pick">
      <div className="ts-lesson__lbl" id={`${id}-label`}>
        {t("knowledge.pick.label")} <span className="ts-lesson__hint">{t("lessons.q.optional")}</span>
      </div>
      {shut && (
        <p className="ts-lesson__bad" role="status">
          {t("knowledge.failure.Forbidden")}
        </p>
      )}
      {!shut && total === 0 && <p className="ts-lesson__hint">{t("knowledge.pick.none")}</p>}
      {!shut && failed && total === null && (
        <p className="ts-lesson__hint" role="status">
          {t("knowledge.pick.failed")}
        </p>
      )}
      {!shut && (chosen.length > 0 || (total !== null && total > 0)) && (
        <div className="ts-lesson__chips" role="group" aria-labelledby={`${id}-label`}>
          {chosen.map((item) => (
            <button
              key={item.id}
              type="button"
              className="ts-lesson__chip ts-lesson__chip--on ts-kg__chip"
              title={item.text}
              aria-label={t("knowledge.pick.remove", { text: item.text })}
              onClick={() => onChange(chosen.filter((other) => other.id !== item.id))}
            >
              {clip(item.text)} ✕
            </button>
          ))}
          <button
            type="button"
            className="ts-btn"
            aria-expanded={picking}
            disabled={full && !picking}
            onClick={() => setPicking((open) => !open)}
          >
            {t("knowledge.pick.add")}
          </button>
        </div>
      )}
      {full && <p className="ts-lesson__hint">{t("knowledge.pick.limit", { max: PICK_MAX })}</p>}
      {picking && !shut && (
        <div className="ts-lesson__pick">
          <input
            className="ts-field__input"
            aria-label={t("knowledge.pick.search")}
            placeholder={t("knowledge.pick.search")}
            value={input}
            autoFocus
            onChange={(event) => setInput(event.target.value)}
          />
          <ul className="ts-lesson__picklist">
            {found !== null && results.length === 0 && (
              <li className="ts-lesson__hint">{t("knowledge.pick.noMatches")}</li>
            )}
            {results.map((item) => (
              <li key={item.id}>
                <button
                  type="button"
                  className="ts-lesson__pickrow"
                  disabled={full}
                  onClick={() => {
                    onChange([...chosen, item]);
                    setInput("");
                  }}
                >
                  {item.text}
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
      {chosen.length > 0 && <p className="ts-lesson__hint">{t("knowledge.pick.hint")}</p>}
    </div>
  );
}
