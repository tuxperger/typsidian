import { useCallback, useEffect, useRef, useState } from "react";
import type {
  GraphForces,
  GraphGroup,
  GraphSettings,
  VaultOverrides,
} from "../wasm-core/typsidian_core.js";

// Настройки графа (фича 025, T022): ключ `graph` в `.typsidian/vault.json` через
// настройки vault 006. Умолчания совпадают с ядром (data-model.md): если ключа
// нет, читаются они. Хук держит рабочую копию, чтобы ползунок откликался сразу,
// а запись в файл шла с прореживанием.

export interface ResolvedGraphSettings {
  filter: string;
  show_orphans: boolean;
  show_attachments: boolean;
  show_tags: boolean;
  arrows: boolean;
  groups: GraphGroup[];
  forces: GraphForces;
  label_zoom: number;
  node_size: number;
}

export const GRAPH_DEFAULTS: ResolvedGraphSettings = {
  filter: "",
  show_orphans: true,
  show_attachments: false,
  show_tags: false,
  arrows: false,
  groups: [],
  forces: { center: 0.3, repel: 0.6, link: 0.5, distance: 0.45 },
  label_zoom: 0.4,
  node_size: 0.5,
};

export function resolveGraphSettings(settings: GraphSettings | undefined): ResolvedGraphSettings {
  if (settings === undefined) return GRAPH_DEFAULTS;
  return {
    filter: settings.filter ?? GRAPH_DEFAULTS.filter,
    show_orphans: settings.show_orphans ?? GRAPH_DEFAULTS.show_orphans,
    show_attachments: settings.show_attachments ?? GRAPH_DEFAULTS.show_attachments,
    show_tags: settings.show_tags ?? GRAPH_DEFAULTS.show_tags,
    arrows: settings.arrows ?? GRAPH_DEFAULTS.arrows,
    groups: settings.groups ?? GRAPH_DEFAULTS.groups,
    forces: { ...GRAPH_DEFAULTS.forces, ...settings.forces },
    label_zoom: settings.label_zoom ?? GRAPH_DEFAULTS.label_zoom,
    node_size: settings.node_size ?? GRAPH_DEFAULTS.node_size,
  };
}

export type SaveState = "idle" | "saved" | "failed";

export interface GraphSettingsState {
  settings: ResolvedGraphSettings;
  update: (patch: Partial<ResolvedGraphSettings>) => void;
  reset: () => void;
  save: SaveState;
}

const SAVE_DELAY_MS = 500;

export function useGraphSettings(
  overrides: VaultOverrides | null,
  write: (graph: GraphSettings) => Promise<boolean>,
): GraphSettingsState {
  const [settings, setSettings] = useState<ResolvedGraphSettings>(() =>
    resolveGraphSettings(overrides?.graph),
  );
  const [save, setSave] = useState<SaveState>("idle");
  const adopted = useRef(overrides !== null);
  const writeRef = useRef(write);
  writeRef.current = write;
  const timer = useRef<number | undefined>(undefined);
  const pending = useRef<ResolvedGraphSettings | null>(null);

  // Vault открылся после первого рендера: берём то, что в его файле.
  useEffect(() => {
    if (overrides !== null && !adopted.current) {
      adopted.current = true;
      setSettings(resolveGraphSettings(overrides.graph));
    }
  }, [overrides]);

  const flush = useCallback(() => {
    window.clearTimeout(timer.current);
    timer.current = undefined;
    const next = pending.current;
    if (next === null) return;
    pending.current = null;
    void writeRef.current(next).then((ok) => setSave(ok ? "saved" : "failed"));
  }, []);

  const schedule = useCallback(
    (next: ResolvedGraphSettings) => {
      pending.current = next;
      setSave("idle");
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(flush, SAVE_DELAY_MS);
    },
    [flush],
  );

  // Незаписанное не теряется при уходе с экрана.
  useEffect(() => flush, [flush]);

  const update = useCallback(
    (patch: Partial<ResolvedGraphSettings>) => {
      setSettings((current) => {
        const next = { ...current, ...patch };
        schedule(next);
        return next;
      });
    },
    [schedule],
  );

  const reset = useCallback(() => {
    setSettings(GRAPH_DEFAULTS);
    schedule(GRAPH_DEFAULTS);
  }, [schedule]);

  return { settings, update, reset, save };
}
