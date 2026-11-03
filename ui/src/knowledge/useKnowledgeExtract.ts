"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { VaultClient } from "../vault-client.js";
import { failureKind, newRequestId } from "../lessons/lessonEnv.js";
import { failureKeyOfKind, type FailureKey } from "./knowledgeText.js";
import { announceKnowledgeJob } from "./useKnowledgeJobs.js";

// Запуск пополнения графа из урока (фича 054, research §8): ядро только говорит «пора», а
// вызов делает клиент — отдельным запросом, которого урок не ждёт. Ответ `Cancelled`
// значит, что задание по этому уроку уже идёт, и молча игнорируется.

export interface KnowledgeExtract {
  /** Запустить пополнение этого урока; идущее уже задание второго не заводит. */
  start: () => void;
  /** Вызов ещё идёт. */
  busy: boolean;
  /** Отказ самого запуска (не отказ посреди задания — тот лежит в `job.failure`). */
  error: FailureKey | null;
}

export function useKnowledgeExtract(
  client: VaultClient,
  vaultId: string,
  path: string,
  /** Вызывается, когда вызов кончился; «занято» снимается после его завершения. */
  onSettled: () => Promise<void> | void,
): KnowledgeExtract {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<FailureKey | null>(null);
  const running = useRef(false);
  const alive = useRef(true);
  const settled = useRef(onSettled);
  settled.current = onSettled;

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const start = useCallback((): void => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setError(null);
    announceKnowledgeJob();
    client.knowledge
      .extract(vaultId, path, newRequestId())
      .catch((failure: unknown) => {
        const kind = failureKind(failure);
        if (kind !== "Cancelled" && alive.current) setError(failureKeyOfKind(kind));
      })
      .finally(async () => {
        announceKnowledgeJob();
        if (!alive.current) {
          running.current = false;
          return;
        }
        // Вид урока перечитывается до снятия «занято»: иначе кнопка «Пополнить граф» мигнёт со
        // старым числом непокрытого текста.
        try {
          await settled.current();
        } finally {
          running.current = false;
          if (alive.current) setBusy(false);
        }
      });
  }, [client, vaultId, path]);

  return { start, busy, error };
}
