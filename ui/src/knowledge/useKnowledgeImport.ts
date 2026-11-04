"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ImportEstimate, KnowledgeJob } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { failureKind, newRequestId } from "../lessons/lessonEnv.js";
import { importFailureKey, type FailureKey } from "./knowledgeText.js";
import { announceKnowledgeJob, useKnowledgeJobs } from "./useKnowledgeJobs.js";

// Импорт заметок в граф (фича 055): оценка → подтверждение → фоновое задание → итог. Что
// уйдёт модели, части, покрытие и итог по файлам решает ядро; здесь — только стадии показа.
// Задание идёт отдельным вызовом, которого интерфейс не ждёт: ход читается опросом
// `knowledge.jobs` (как у 054), редактирование заметок не блокируется (SC-003).

export type ImportPhase =
  | { kind: "idle" }
  | { kind: "estimating"; paths: string[] }
  /** Оценка получена; `error` — отказ запуска (устаревшая оценка, второй импорт и т. п.). */
  | { kind: "confirm"; paths: string[]; estimate: ImportEstimate; error: FailureKey | null; stale: boolean }
  /** Оценку получить не удалось (корень vault, вне vault, больше 200 путей, ИИ выключен). */
  | { kind: "refused"; paths: string[]; error: FailureKey }
  | { kind: "running"; requestId: string; estimate: ImportEstimate }
  | { kind: "done"; requestId: string; estimate: ImportEstimate; job: KnowledgeJob | null };

export interface KnowledgeImport {
  phase: ImportPhase;
  /** Задание, которое сейчас идёт или только что закончилось (по `requestId`). */
  job: KnowledgeJob | null;
  /** Оценить выбранные пути и показать подтверждение. */
  begin: (paths: readonly string[]) => void;
  /** Подтвердить: запустить импорт с показанной оценкой, как есть. */
  start: () => void;
  /** Закрыть подтверждение или окно итога. */
  dismiss: () => void;
  /** Отменить идущий импорт: оставшиеся файлы не отправляются, готовое остаётся. */
  cancel: () => void;
  /** Повтор файла итога: новая оценка этого пути и подтверждение. */
  retry: (path: string) => void;
}

export function useKnowledgeImport(client: VaultClient | null, vaultId: string): KnowledgeImport {
  const [phase, setPhase] = useState<ImportPhase>({ kind: "idle" });
  const { jobs } = useKnowledgeJobs(client, vaultId);
  const alive = useRef(true);
  // Номер попытки: ответ устаревшей оценки не должен перебить более новую.
  const attempt = useRef(0);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const estimate = useCallback(
    (paths: string[]): void => {
      if (client === null) return;
      const mine = ++attempt.current;
      setPhase({ kind: "estimating", paths });
      client.knowledge
        .importEstimate(paths)
        .then((found) => {
          if (!alive.current || mine !== attempt.current) return;
          setPhase({ kind: "confirm", paths, estimate: found, error: null, stale: false });
        })
        .catch((failure: unknown) => {
          if (!alive.current || mine !== attempt.current) return;
          setPhase({ kind: "refused", paths, error: importFailureKey(failure) });
        });
    },
    [client],
  );

  const begin = useCallback((paths: readonly string[]): void => estimate([...paths]), [estimate]);

  const start = useCallback((): void => {
    if (client === null || phase.kind !== "confirm") return;
    const { estimate: confirmed, paths } = phase;
    const requestId = newRequestId();
    const mine = ++attempt.current;
    setPhase({ kind: "running", requestId, estimate: confirmed });
    announceKnowledgeJob();
    client.knowledge
      .import(vaultId, requestId, confirmed)
      .then((job) => {
        if (!alive.current || mine !== attempt.current) return;
        setPhase({ kind: "done", requestId, estimate: confirmed, job });
      })
      .catch(async (failure: unknown) => {
        if (!alive.current || mine !== attempt.current) return;
        // Отказ уже после заведения задания: итог лежит в реестре ядра.
        let known: KnowledgeJob | undefined;
        try {
          known = (await client.knowledge.jobs(vaultId)).find((item) => item.id === requestId);
        } catch {
          known = undefined;
        }
        if (!alive.current || mine !== attempt.current) return;
        if (known !== undefined && known.state !== "Running") {
          setPhase({ kind: "done", requestId, estimate: confirmed, job: known });
          return;
        }
        // Задание не заведено: ядро отказало до записи журнала. Оценку устаревшей считает оно.
        setPhase({
          kind: "confirm",
          paths,
          estimate: confirmed,
          error: importFailureKey(failure),
          stale: failureKind(failure) === "LessonChanged",
        });
      })
      .finally(announceKnowledgeJob);
  }, [client, vaultId, phase]);

  const dismiss = useCallback((): void => {
    attempt.current += 1;
    setPhase({ kind: "idle" });
  }, []);

  const cancel = useCallback((): void => {
    if (client === null || phase.kind !== "running") return;
    client.ai.cancel(vaultId, phase.requestId);
  }, [client, vaultId, phase]);

  const retry = useCallback((path: string): void => estimate([path]), [estimate]);

  const requestId = phase.kind === "running" || phase.kind === "done" ? phase.requestId : null;
  const job = useMemo<KnowledgeJob | null>(() => {
    if (requestId === null) return null;
    const polled = jobs.find((item) => item.id === requestId) ?? null;
    if (phase.kind === "done" && phase.job !== null) return phase.job;
    return polled;
  }, [jobs, requestId, phase]);

  return { phase, job, begin, start, dismiss, cancel, retry };
}
