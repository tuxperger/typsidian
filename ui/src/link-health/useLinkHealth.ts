"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type {
  BrokenSite,
  BrokenTarget,
  LinkFix,
  LinkHealth,
  OpenText,
  UndoRecord,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import type { OpenNoteEditors } from "../note-panel/links/openNoteEditors.js";

// Состояние ссылок vault (фича 028). Здесь только ритм: когда спросить ядро,
// в каком порядке чинить заметки группы и что показать в итоге. Что битое,
// что сирота, как переписать ссылку и свеж ли текст — решает ядро
// (`links::health`, принцип I).

/** Пауза после изменения vault перед пересчётом — как у колонки связей 013. */
export const HEALTH_DEBOUNCE_MS = 600;

/** Что сделано последней починкой (макет L1, панель итога). */
export interface FixReport {
  fix: LinkFix["kind"] | "Create";
  /** Куда перенаправлено — название. */
  to: string | null;
  /** Сколько ссылок исправлено; `null` — часть заметок пропущена, число неизвестно. */
  links: number | null;
  notes: string[];
  skipped: string[];
  readOnly: boolean;
}

export interface LinkHealthState {
  health: LinkHealth | null;
  busy: boolean;
  report: FixReport | null;
  canUndo: boolean;
  /** Починить все ссылки группы во всех её заметках. */
  fixGroup: (target: BrokenTarget, fix: LinkFix) => Promise<void>;
  /** Убрать одну ссылку. */
  unlinkSite: (target: BrokenTarget, site: BrokenSite) => Promise<void>;
  /** Создать заметку или доску по пути цели (как открытие ссылки 013/018). */
  create: (target: BrokenTarget) => Promise<void>;
  undo: () => Promise<void>;
  /** Отметить заметку «намеренно без ссылок» или снять отметку. */
  setIntentional: (path: string, on: boolean) => Promise<void>;
  dismissReport: () => void;
}

export interface UseLinkHealthInput {
  client: VaultClient | null;
  editors: OpenNoteEditors;
  openTexts: () => readonly OpenText[];
  /** Растёт на изменения vault — пересчёт с паузой (FR-005). */
  revision: number;
  /** Создать файл; `true` — создан. */
  createFile: (path: string) => Promise<boolean>;
  /** Файлы изменены здесь — колонка связей и прочие пересчитываются. */
  onChanged: (paths: readonly string[]) => void;
  /** Настройки vault переписаны — перечитать. */
  onSettingsChanged: () => void;
}

const nameOf = (path: string): string =>
  path
    .split("/")
    .pop()
    ?.replace(/\.(typ|excalidraw)$/, "") ?? path;

export function useLinkHealth({
  client,
  editors,
  openTexts,
  revision,
  createFile,
  onChanged,
  onSettingsChanged,
}: UseLinkHealthInput): LinkHealthState {
  const [health, setHealth] = useState<LinkHealth | null>(null);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<FixReport | null>(null);
  const [batch, setBatch] = useState<UndoRecord[]>([]);
  const [tick, setTick] = useState(0);
  const asked = useRef(0);
  const openTextsRef = useRef(openTexts);
  openTextsRef.current = openTexts;

  useEffect(() => {
    if (client === null) {
      setHealth(null);
      return;
    }
    const mine = ++asked.current;
    const timer = window.setTimeout(() => {
      void client.links
        .health([...openTextsRef.current()])
        .then((found) => {
          if (mine === asked.current) setHealth(found);
        })
        .catch(() => undefined);
    }, HEALTH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [client, revision, tick]);

  const refresh = useCallback(() => setTick((value) => value + 1), []);

  /** Чинит заметки по одной; `only` — одна запись в одной заметке. */
  const run = useCallback(
    async (
      target: BrokenTarget,
      fix: LinkFix,
      notes: readonly { path: string; digest: string; only?: number }[],
      links: number,
    ) => {
      if (client === null || busy) return;
      setBusy(true);
      const done: FixReport = {
        fix: fix.kind,
        to: fix.kind === "Redirect" ? nameOf(fix.to) : null,
        links: 0,
        notes: [],
        skipped: [],
        readOnly: false,
      };
      const undo: UndoRecord[] = [];
      const changed: string[] = [];
      for (const note of notes) {
        const outcome = await client.links.fix(
          {
            source: note.path,
            target: target.target,
            digest: note.digest,
            fix,
            ...(note.only === undefined ? {} : { only: note.only }),
          },
          editors.textOf(note.path),
        );
        switch (outcome.kind) {
          case "Written":
            undo.push(outcome.undo);
            done.notes.push(nameOf(note.path));
            changed.push(note.path);
            break;
          case "EditorEdit":
            if (editors.applyEdit(note.path, outcome.edit).ok) {
              undo.push(outcome.undo);
              done.notes.push(nameOf(note.path));
              changed.push(note.path);
            } else {
              done.skipped.push(nameOf(note.path));
            }
            break;
          case "ReadOnly":
            done.readOnly = true;
            break;
          case "Stale":
          case "UnsavedElsewhere":
          case "Failed":
          case "NothingSelected":
            done.skipped.push(nameOf(note.path));
            break;
          default: {
            const exhaustive: never = outcome;
            return exhaustive;
          }
        }
        if (done.readOnly) break;
      }
      done.links = done.skipped.length === 0 && !done.readOnly ? links : null;
      setBatch(undo);
      setReport(done);
      setBusy(false);
      if (changed.length > 0) onChanged(changed);
      refresh();
    },
    [busy, client, editors, onChanged, refresh],
  );

  const fixGroup = useCallback(
    (target: BrokenTarget, fix: LinkFix) => run(target, fix, target.notes, target.links),
    [run],
  );

  const unlinkSite = useCallback(
    (target: BrokenTarget, site: BrokenSite) =>
      run(
        target,
        { kind: "Unlink" },
        [{ path: site.source, digest: site.digest, only: site.index }],
        1,
      ),
    [run],
  );

  const create = useCallback(
    async (target: BrokenTarget) => {
      if (await createFile(target.target)) {
        setBatch([]);
        setReport({
          fix: "Create",
          to: target.title,
          links: target.links,
          notes: [],
          skipped: [],
          readOnly: false,
        });
        onChanged([target.target]);
        refresh();
      }
    },
    [createFile, onChanged, refresh],
  );

  const undo = useCallback(async () => {
    if (client === null || batch.length === 0) return;
    const changed: string[] = [];
    for (const record of [...batch].reverse()) {
      const outcome = await client.replaceUndo(record, editors.textOf(record.path));
      if (outcome.kind === "Restored") changed.push(record.path);
      else if (outcome.kind === "EditorEdit" && editors.applyEdit(record.path, outcome.edit).ok) {
        changed.push(record.path);
      }
    }
    setBatch([]);
    setReport(null);
    if (changed.length > 0) onChanged(changed);
    refresh();
  }, [batch, client, editors, onChanged, refresh]);

  const setIntentional = useCallback(
    async (path: string, on: boolean) => {
      if (client === null) return;
      const settings = await client.vaultSettings();
      const current = settings.intentional_orphans ?? [];
      const next = on
        ? [...current.filter((entry) => entry !== path), path]
        : current.filter((entry) => entry !== path);
      await client.setVaultSettings({ ...settings, intentional_orphans: next });
      onSettingsChanged();
      refresh();
    },
    [client, onSettingsChanged, refresh],
  );

  return {
    health,
    busy,
    report,
    canUndo: batch.length > 0,
    fixGroup,
    unlinkSite,
    create,
    undo,
    setIntentional,
    dismissReport: () => setReport(null),
  };
}
