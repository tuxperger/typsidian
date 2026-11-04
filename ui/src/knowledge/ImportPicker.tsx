"use client";

import { useId, useMemo, useState } from "react";
import type { Note } from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { IMPORT_MAX_PATHS } from "./knowledgeText.js";

// Выбор файлов и папок для импорта из вида «Граф знаний» (фича 055, FR-001): явный выбор,
// всего vault одним действием нет. Что из выбранного уйдёт модели, решает оценка ядра на
// следующем шаге — здесь отбираются только пути.

const ROWS_SHOWN = 200;

interface Entry {
  path: string;
  folder: boolean;
  /** Файлов внутри папки. */
  files: number;
}

export interface ImportPickerProps {
  notes: readonly Note[];
  onPick: (paths: string[]) => void;
  onCancel: () => void;
}

/** Файлы, которые имеет смысл предлагать: заметки и прочие файлы vault (текст, PDF). */
function isOffered(note: Note): boolean {
  return note.kind === "Note" || note.kind === "Other";
}

function entriesOf(notes: readonly Note[]): Entry[] {
  const folders = new Map<string, number>();
  const files: Entry[] = [];
  for (const note of notes) {
    if (!isOffered(note)) continue;
    files.push({ path: note.path, folder: false, files: 1 });
    const parts = note.path.split("/");
    for (let depth = 1; depth < parts.length; depth += 1) {
      const folder = parts.slice(0, depth).join("/");
      folders.set(folder, (folders.get(folder) ?? 0) + 1);
    }
  }
  const all: Entry[] = [
    ...[...folders].map(([path, count]): Entry => ({ path, folder: true, files: count })),
    ...files,
  ];
  return all.sort((a, b) => a.path.localeCompare(b.path));
}

export function ImportPicker({ notes, onPick, onCancel }: ImportPickerProps): React.JSX.Element {
  const { t } = useI18n();
  const id = useId();
  const [query, setQuery] = useState("");
  const [chosen, setChosen] = useState<readonly string[]>([]);
  useDismissOnEscape(onCancel);

  const entries = useMemo(() => entriesOf(notes), [notes]);
  const shown = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return entries.filter((entry) => needle === "" || entry.path.toLowerCase().includes(needle)).slice(0, ROWS_SHOWN);
  }, [entries, query]);

  const toggle = (path: string): void =>
    setChosen((list) => (list.includes(path) ? list.filter((item) => item !== path) : [...list, path]));
  const full = chosen.length >= IMPORT_MAX_PATHS;

  return (
    <div className="ts-overlay ts-overlay--center" role="presentation">
      <section
        className="ts-lesson-dialog ts-import"
        role="dialog"
        aria-modal="true"
        aria-labelledby={`${id}-title`}
      >
        <h2 className="ts-lesson-dialog__title" id={`${id}-title`}>
          {t("knowledge.import.pick.title")}
        </h2>
        {/* eslint-disable-next-line jsx-a11y/no-autofocus -- окно открыто ради выбора */}
        <input
          className="ts-field__input"
          type="search"
          autoFocus
          aria-label={t("knowledge.import.pick.search")}
          placeholder={t("knowledge.import.pick.search")}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <ul className="ts-lesson__picklist ts-import__picklist" aria-label={t("knowledge.import.pick.title")}>
          {shown.length === 0 && <li className="ts-lesson__hint">{t("knowledge.import.pick.none")}</li>}
          {shown.map((entry) => {
            const on = chosen.includes(entry.path);
            return (
              <li key={`${entry.folder ? "d" : "f"}:${entry.path}`}>
                <label className="ts-import__pickrow">
                  <input
                    type="checkbox"
                    checked={on}
                    disabled={!on && full}
                    onChange={() => toggle(entry.path)}
                  />
                  {entry.folder && <Icon name="folder" size={13} />}
                  <span className="ts-import__name">{entry.path}</span>
                  {entry.folder && (
                    <span className="ts-lesson__hint">{t("knowledge.import.pick.folder", { count: entry.files })}</span>
                  )}
                </label>
              </li>
            );
          })}
        </ul>
        <p className="ts-lesson__hint">{t("knowledge.import.pick.hint")}</p>
        {full && (
          <p className="ts-lesson__hint" role="status">
            {t("knowledge.import.pick.limit", { max: IMPORT_MAX_PATHS })}
          </p>
        )}
        <div className="ts-lesson__acts">
          <span className="ts-lesson__acts-hint" aria-live="polite">
            {t("knowledge.import.pick.chosen", { count: chosen.length })}
          </span>
          <button type="button" className="ts-btn" onClick={onCancel}>
            {t("knowledge.import.cancel")}
          </button>
          <button
            type="button"
            className="ts-btn ts-btn--accent"
            disabled={chosen.length === 0}
            onClick={() => onPick([...chosen])}
          >
            {t("knowledge.import.pick.next")}
          </button>
        </div>
      </section>
    </div>
  );
}
