"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { KnowledgeJob } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";

// Задания пополнения графа (фича 054, FR-004): интерфейс опрашивает `knowledge.jobs`, пока
// какое-то идёт, и ещё несколько секунд после запуска — запись задания в реестр ядра
// появляется чуть позже вызова. Без идущих заданий опроса нет.

const POLL_MS = 700;
const GRACE_MS = 4000;
const KICK_EVENT = "typsidian:knowledge-kick";

/** Сообщает всем открытым спискам заданий, что задание запущено или кончилось. */
export function announceKnowledgeJob(): void {
  window.dispatchEvent(new Event(KICK_EVENT));
}

export interface KnowledgeJobsState {
  jobs: KnowledgeJob[];
  /** Перечитать сейчас и опрашивать дальше. */
  kick: () => void;
}

export function useKnowledgeJobs(client: VaultClient | null, vaultId: string): KnowledgeJobsState {
  const [jobs, setJobs] = useState<KnowledgeJob[]>([]);
  const graceUntil = useRef(0);
  const timer = useRef<number | undefined>(undefined);
  const alive = useRef(true);

  const refresh = useCallback(async (): Promise<void> => {
    if (client === null) return;
    window.clearTimeout(timer.current);
    let list: KnowledgeJob[] = [];
    try {
      list = await client.knowledge.jobs(vaultId);
    } catch {
      // Реестр недоступен — просто нет заданий.
    }
    if (!alive.current) return;
    setJobs(list);
    if (list.some((job) => job.state === "Running") || Date.now() < graceUntil.current) {
      timer.current = window.setTimeout(() => void refresh(), POLL_MS);
    }
  }, [client, vaultId]);

  const kick = useCallback((): void => {
    graceUntil.current = Date.now() + GRACE_MS;
    void refresh();
  }, [refresh]);

  useEffect(() => {
    alive.current = true;
    void refresh();
    window.addEventListener(KICK_EVENT, kick);
    return () => {
      alive.current = false;
      window.clearTimeout(timer.current);
      window.removeEventListener(KICK_EVENT, kick);
    };
  }, [refresh, kick]);

  return { jobs, kick };
}
