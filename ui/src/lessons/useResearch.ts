"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { AiFailure, ResearchKind, ResearchView } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { failureKind, newRequestId } from "./lessonEnv.js";
import { isActive } from "./researchText.js";

// Исследования урока (фича 056): запуск, отмена, повтор и опрос вида, пока что-то идёт.
// Цикл запросов ведёт ядро; здесь решается только «когда позвать»: карточка с `launch_due`
// запускается один раз (id в `launched` живёт дольше вкладки — повторное открытие урока
// не должно слать тот же запрос ещё раз), остальное заказывает пользователь.

const POLL_MS = 1200;

/** Запущенные клиентом карточки учителя: `vault/id`. */
const launched = new Set<string>();
/** Карточки, чью стоимость пользователь не подтвердил (до перезагрузки окна). */
const declinedIds = new Set<string>();

export interface ResearchFlow {
  /** Запрос пользователя («В вебе», «В PDF…»). */
  start: (kind: ResearchKind, question: string, pdf?: string) => void;
  cancel: (id: string) => void;
  retry: (item: ResearchView) => void;
  /** Подтверждённый запуск PDF, который просил учитель. */
  confirm: (item: ResearchView) => void;
  decline: (item: ResearchView) => void;
  isDeclined: (id: string) => boolean;
  /** Отказ запуска до заведения карточки. */
  error: AiFailure["kind"] | "Unknown" | null;
}

export function useResearch(
  client: VaultClient,
  vaultId: string,
  path: string,
  items: readonly ResearchView[],
  /** Перечитать вид урока; вызывается раз в секунду, пока что-то идёт, и после запуска. */
  refresh: () => Promise<void>,
): ResearchFlow {
  const [busy, setBusy] = useState(0);
  const [error, setError] = useState<ResearchFlow["error"]>(null);
  const [, setDeclinedTick] = useState(0);
  const alive = useRef(true);
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const run = useCallback(
    (id: string, kind: ResearchKind, question: string, pdf: string | undefined): void => {
      setBusy((count) => count + 1);
      client.research
        .start(vaultId, path, id, kind, question, pdf)
        .catch((failure: unknown) => {
          const found = failureKind(failure);
          if (found !== "Cancelled" && alive.current) setError(found ?? "Unknown");
        })
        .finally(() => {
          if (alive.current) setBusy((count) => count - 1);
          void refreshRef.current();
        });
    },
    [client, vaultId, path],
  );

  const start = useCallback(
    (kind: ResearchKind, question: string, pdf?: string): void => {
      setError(null);
      run(newRequestId(), kind, question, pdf);
    },
    [run],
  );

  const launch = useCallback(
    (item: ResearchView): void => {
      const key = `${vaultId}/${item.id}`;
      if (launched.has(key)) return;
      launched.add(key);
      run(item.id, item.kind, item.question, item.path);
    },
    [run, vaultId],
  );

  // Учитель запросил исследование: запускаем один раз. PDF ждёт подтверждения стоимости.
  useEffect(() => {
    for (const item of items) {
      if (item.launch_due && !item.confirm_cost) launch(item);
    }
  }, [items, launch]);

  const active = busy > 0 || items.some((item) => isActive(item) || item.launch_due);
  useEffect(() => {
    if (!active) return;
    let polling = false;
    const timer = window.setInterval(() => {
      if (polling) return;
      polling = true;
      void refreshRef.current().finally(() => {
        polling = false;
      });
    }, POLL_MS);
    return () => window.clearInterval(timer);
  }, [active]);

  const cancel = useCallback((id: string): void => client.ai.cancel(vaultId, id), [client, vaultId]);
  const retry = useCallback(
    (item: ResearchView): void => start(item.kind, item.question, item.path),
    [start],
  );
  const confirm = useCallback((item: ResearchView): void => launch(item), [launch]);
  const decline = useCallback((item: ResearchView): void => {
    declinedIds.add(`${vaultId}/${item.id}`);
    setDeclinedTick((tick) => tick + 1);
  }, [vaultId]);
  const isDeclined = useCallback((id: string): boolean => declinedIds.has(`${vaultId}/${id}`), [vaultId]);

  return { start, cancel, retry, confirm, decline, isDeclined, error };
}
