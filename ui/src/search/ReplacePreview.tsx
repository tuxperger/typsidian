"use client";

import type { ReplaceNote, ReplacePreview as Preview } from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import type { Selection } from "./useReplace.js";

// Выдача режима замены (фича 027, макет R1): каждое совпадение с отметкой и
// строкой «было → станет». Куски строки режет ядро; здесь только разметка.

export interface ReplacePreviewProps {
  preview: Preview;
  selection: Selection;
  onToggleMatch: (path: string, index: number) => void;
  onToggleNote: (path: string) => void;
  onOpen: (path: string) => void;
}

interface Folder {
  folder: string;
  notes: ReplaceNote[];
  matches: number;
}

function byFolder(notes: readonly ReplaceNote[]): Folder[] {
  const folders: Folder[] = [];
  for (const note of notes) {
    const found = folders.find((entry) => entry.folder === note.folder);
    if (found) {
      found.notes.push(note);
      found.matches += note.matches.length;
    } else {
      folders.push({ folder: note.folder, notes: [note], matches: note.matches.length });
    }
  }
  return folders;
}

export function ReplacePreview({
  preview,
  selection,
  onToggleMatch,
  onToggleNote,
  onOpen,
}: ReplacePreviewProps): React.JSX.Element {
  const { t } = useI18n();
  return (
    <>
      {byFolder(preview.notes).map((group) => (
        <div key={group.folder || "/"} className="ts-search__group">
          <div className="ts-search__group-head">
            <Icon name="folder" size={12} />
            {group.folder || t("search.vaultRoot")}
            <span className="ts-search__group-meta">
              {t("search.notes", { count: group.notes.length })} ·{" "}
              {t("search.matches", { count: group.matches })}
            </span>
          </div>
          {group.notes.map((note) => {
            const marks = selection.get(note.path);
            const selected = marks?.size ?? 0;
            const all = selected === note.matches.length;
            return (
              <div key={note.path} className="ts-replace__note">
                <label className="ts-replace__head">
                  <input
                    type="checkbox"
                    className="ts-replace__check"
                    checked={all}
                    ref={(input) => {
                      if (input) input.indeterminate = selected > 0 && !all;
                    }}
                    onChange={() => onToggleNote(note.path)}
                    aria-label={t("replace.selectNote", { name: note.name })}
                  />
                  <span className="ts-search__hit-name">{note.name}</span>
                  <span className="ts-search__hit-meta">
                    {t("replace.noteMeta", { selected, total: note.matches.length })}
                    {note.open_in_editor && ` · ${t("replace.inEditor")}`}
                  </span>
                </label>
                {note.matches.map((match) => {
                  const on = marks?.has(match.index) ?? false;
                  return (
                    <div
                      key={match.index}
                      className={on ? "ts-replace__match" : "ts-replace__match ts-replace__match--off"}
                    >
                      <input
                        type="checkbox"
                        className="ts-replace__check"
                        checked={on}
                        onChange={() => onToggleMatch(note.path, match.index)}
                        onKeyDown={(event) => {
                          // ⇧Пробел — вся заметка (макет R1).
                          if (event.key === " " && event.shiftKey) {
                            event.preventDefault();
                            onToggleNote(note.path);
                          }
                        }}
                        aria-label={t("replace.selectMatch", { line: match.line })}
                      />
                      <span className="ts-replace__line">{match.line}</span>
                      <button
                        type="button"
                        className="ts-replace__text"
                        onClick={() => onOpen(note.path)}
                      >
                        {match.pieces.map((piece, index) =>
                          piece.kind === "Removed" ? (
                            <del key={index} className="ts-replace__del">
                              {piece.text}
                            </del>
                          ) : piece.kind === "Inserted" ? (
                            <ins key={index} className="ts-replace__ins">
                              {piece.text}
                            </ins>
                          ) : (
                            <span key={index}>{piece.text}</span>
                          ),
                        )}
                      </button>
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>
      ))}
    </>
  );
}
