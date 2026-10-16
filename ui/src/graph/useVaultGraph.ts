import { useEffect, useMemo, useRef, useState } from "react";
import type { GraphRequest, VaultGraph } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useVaultEvents } from "../vault-tree/useVaultEvents.js";
import type { ResolvedGraphSettings } from "./useGraphSettings.js";

// Запрос графа у ядра и его обновление по событиям vault (фича 025, T014,
// research.md §8): повторный `vault_graph` с прореживанием 300 мс.

export interface LocalScope {
  center: string;
  depth: number;
}

export interface LoadedGraph {
  graph: VaultGraph;
  /** Переименования, случившиеся с прошлого графа: положение переходит новому id. */
  renames: [string, string][];
}

export interface VaultGraphState {
  loaded: LoadedGraph | null;
  loading: boolean;
  failed: boolean;
}

const DEBOUNCE_MS = 300;

export function buildRequest(
  settings: ResolvedGraphSettings,
  local: LocalScope | null,
): GraphRequest {
  return {
    filter: settings.filter,
    show_orphans: settings.show_orphans,
    show_attachments: settings.show_attachments,
    show_tags: settings.show_tags,
    groups: settings.groups,
    ...(local !== null ? { local: { center: local.center, depth: local.depth } } : {}),
  };
}

export function useVaultGraph(client: VaultClient | null, request: GraphRequest): VaultGraphState {
  const [loaded, setLoaded] = useState<LoadedGraph | null>(null);
  const [loading, setLoading] = useState(true);
  const [failed, setFailed] = useState(false);
  const [version, setVersion] = useState(0);
  const renames = useRef<[string, string][]>([]);
  const bump = useRef<number | undefined>(undefined);
  const sequence = useRef(0);
  const first = useRef(true);
  const seenVersion = useRef(0);
  const keyRef = useRef("");
  const applied = useRef(0);

  const key = useMemo(() => JSON.stringify(request), [request]);
  keyRef.current = key;

  useVaultEvents(client, {
    onNoteCreated: () => schedule(),
    onNoteDeleted: () => schedule(),
    onNoteUpdated: () => schedule(),
    onNoteRenamed: (from, to) => {
      renames.current.push([from, to]);
      schedule();
    },
    onLinksInvalidated: () => schedule(),
  });

  function schedule(): void {
    window.clearTimeout(bump.current);
    bump.current = window.setTimeout(() => setVersion((v) => v + 1), DEBOUNCE_MS);
  }

  useEffect(
    () => () => {
      window.clearTimeout(bump.current);
    },
    [],
  );

  useEffect(() => {
    if (client === null) return;
    const mine = ++sequence.current;
    const run = (): void => {
      setLoading(true);
      client
        .graph(JSON.parse(key) as GraphRequest)
        .then((graph) => {
          // Ответ на прежний запрос годится, пока не сменился сам запрос: событие vault
          // не должно отбрасывать уже готовый граф и заставлять ждать второй.
          if (key !== keyRef.current || mine < applied.current) return;
          applied.current = mine;
          const taken = renames.current;
          renames.current = [];
          setLoaded({ graph, renames: taken });
          setFailed(false);
          setLoading(false);
        })
        .catch(() => {
          if (key !== keyRef.current || mine < applied.current) return;
          setFailed(true);
          setLoading(false);
        });
    };
    // Первый показ — сразу; правка фильтра и события vault — с прореживанием.
    if (first.current) {
      first.current = false;
      run();
      return;
    }
    // Версия поднялась — прореживание уже отстояно в `schedule`.
    const delay = version !== seenVersion.current ? 0 : DEBOUNCE_MS;
    seenVersion.current = version;
    const timer = window.setTimeout(run, delay);
    return () => window.clearTimeout(timer);
  }, [client, key, version]);

  return { loaded, loading, failed };
}
