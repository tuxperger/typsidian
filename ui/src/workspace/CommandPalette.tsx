"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Icon, type IconName } from "../icons/Icons.js";
import { plural } from "../plural.js";
import type { Note } from "../wasm-core/typsidian_core.js";

// Палитра команд ⌘K — экран 3 макета (handoff § Screens 3).
//
// Что здесь есть и чего нет: фильтрация, навигация с клавиатуры и вызов
// действия. Списка команд она не придумывает — он приходит сверху, из
// `Workspace`, где эти действия и живут. Иначе палитра стала бы вторым местом,
// знающим, что умеет приложение.

export interface PaletteCommand {
  id: string;
  title: string;
  /** Подсказка справа — сочетание клавиш; пустая, если его нет. */
  meta?: string;
  icon: IconName;
  run: () => void;
}

export interface CommandPaletteProps {
  notes: Note[];
  commands: PaletteCommand[];
  onOpenNote: (note: Note) => void;
  onClose: () => void;
}

/** Строка списка после фильтрации — заметка или команда, но не то и другое. */
type Row =
  | { kind: "note"; key: string; title: string; meta: string; note: Note }
  | { kind: "command"; key: string; title: string; meta: string; command: PaletteCommand };

export function CommandPalette({
  notes,
  commands,
  onOpenNote,
  onClose,
}: CommandPaletteProps): React.JSX.Element {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLUListElement | null>(null);

  const noteRows = useMemo<Row[]>(() => {
    const needle = query.trim().toLowerCase();
    return notes
      .filter((note) => note.name.toLowerCase().includes(needle))
      .map((note) => ({
        kind: "note" as const,
        key: `note:${note.path}`,
        title: note.name,
        meta: describeChangedAt(note.stamp.mtime_millis),
        note,
      }));
  }, [notes, query]);

  const commandRows = useMemo<Row[]>(() => {
    const needle = query.trim().toLowerCase();
    return commands
      .filter((command) => command.title.toLowerCase().includes(needle))
      .map((command) => ({
        kind: "command" as const,
        key: `command:${command.id}`,
        title: command.title,
        meta: command.meta ?? "",
        command,
      }));
  }, [commands, query]);

  const rows = useMemo(() => [...noteRows, ...commandRows], [noteRows, commandRows]);

  // Фильтр сузился — прежний индекс мог указывать за конец списка.
  useEffect(() => {
    setActive((current) => (current < rows.length ? current : 0));
  }, [rows.length]);

  const runRow = useCallback(
    (row: Row | undefined) => {
      if (!row) return;
      // Палитра закрывается до действия: открытие заметки меняет то, что под
      // ней, и оставлять поверх этого модалку незачем.
      onClose();
      if (row.kind === "note") onOpenNote(row.note);
      else row.command.run();
    },
    [onClose, onOpenNote],
  );

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setActive((current) => (rows.length === 0 ? 0 : (current + 1) % rows.length));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setActive((current) =>
          rows.length === 0 ? 0 : (current - 1 + rows.length) % rows.length,
        );
      } else if (event.key === "Enter") {
        event.preventDefault();
        runRow(rows[active]);
      }
      // Escape намеренно не здесь: он закрывает палитру из любого места, и
      // слушатель на него висит в `Workspace` вместе с ⌘K.
    },
    [rows, active, runRow],
  );

  // Выбранная строка обязана оставаться видимой при навигации с клавиатуры.
  useEffect(() => {
    const list = listRef.current;
    const selected = list?.querySelector<HTMLElement>('[aria-selected="true"]');
    selected?.scrollIntoView({ block: "nearest" });
  }, [active]);

  return (
    <div
      className="ts-palette-scrim"
      role="presentation"
      onMouseDown={(event) => {
        // Только по самому затемнению: нажатие внутри модалки её не закрывает.
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className="ts-palette"
        role="dialog"
        aria-modal="true"
        aria-label="Палитра команд"
        onKeyDown={onKeyDown}
      >
        <div className="ts-palette__search">
          <Icon name="search" size={15} />
          {/* eslint-disable-next-line jsx-a11y/no-autofocus -- палитра
              открывается по ⌘K ради ввода; фокус в поле — это и есть команда */}
          {/* Поле — combobox над списком: навигация идёт стрелками, а фокус
              остаётся во вводе, поэтому выбранную строку называет
              `aria-activedescendant`, а не фокус. */}
          <input
            autoFocus
            type="text"
            role="combobox"
            aria-expanded="true"
            aria-controls="ts-palette-list"
            aria-activedescendant={rows[active] ? rowId(rows[active]) : undefined}
            value={query}
            aria-label="Введите команду или название заметки"
            placeholder="Введите команду или название заметки…"
            onChange={(event) => {
              setQuery(event.target.value);
              setActive(0);
            }}
          />
          <button
            type="button"
            className="ts-btn ts-btn--ghost"
            aria-label="Закрыть палитру"
            onClick={onClose}
          >
            <Icon name="x" size={12} />
          </button>
        </div>

        <ul className="ts-palette__list" id="ts-palette-list" role="listbox" ref={listRef}>
          {noteRows.length > 0 && (
            <li className="ts-palette__group" role="presentation">
              Заметки
            </li>
          )}
          {noteRows.map((row) => (
            <PaletteRow
              key={row.key}
              row={row}
              selected={rows[active]?.key === row.key}
              onRun={() => runRow(row)}
            />
          ))}

          {commandRows.length > 0 && (
            <li className="ts-palette__group" role="presentation">
              Команды
            </li>
          )}
          {commandRows.map((row) => (
            <PaletteRow
              key={row.key}
              row={row}
              selected={rows[active]?.key === row.key}
              onRun={() => runRow(row)}
            />
          ))}

          {rows.length === 0 && (
            <li className="ts-palette__empty" role="presentation">
              ничего не найдено
            </li>
          )}
        </ul>

        <div className="ts-palette__hints">
          <span>
            <Icon name="updown" size={11} />
            навигация
          </span>
          <span>
            <Icon name="enter" size={11} />
            открыть
          </span>
          <span>
            <Icon name="x" size={11} />
            esc закрыть
          </span>
        </div>
      </div>
    </div>
  );
}

/** Идентификатор строки — по нему `aria-activedescendant` называет выбранную. */
function rowId(row: Row): string {
  return `ts-palette-${row.key.replace(/[^\w-]/g, "-")}`;
}

function PaletteRow({
  row,
  selected,
  onRun,
}: {
  row: Row;
  selected: boolean;
  onRun: () => void;
}): React.JSX.Element {
  return (
    <li
      id={rowId(row)}
      role="option"
      aria-selected={selected}
      className={
        row.kind === "command"
          ? "ts-palette__row ts-palette__row--command"
          : "ts-palette__row"
      }
      onClick={onRun}
    >
      <Icon
        name={row.kind === "command" ? row.command.icon : "file"}
        className="ts-palette__icon"
      />
      <span className="ts-palette__title">{row.title}</span>
      {row.meta && <span className="ts-palette__meta">{row.meta}</span>}
    </li>
  );
}

/**
 * Когда файл менялся, словами.
 *
 * Ноль означает «отметки времени нет»: web-бэкенд намеренно не использует
 * `lastModified` как часть идентичности заметки (`stamp_from_content` в
 * `packages/core/src/vault/web.rs`), и показывать вместо этого «55 лет назад»
 * было бы враньём. В таком случае мета просто пустая.
 */
function describeChangedAt(mtimeMillis: number): string {
  if (!mtimeMillis) return "";
  const elapsed = Date.now() - mtimeMillis;
  if (elapsed < 0) return "";

  // `plural` уже подставляет само число — снаружи его дублировать нельзя.
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return "изменено сейчас";
  if (minutes < 60) return `${plural(minutes, "минуту", "минуты", "минут")} назад`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${plural(hours, "час", "часа", "часов")} назад`;

  const days = Math.floor(hours / 24);
  return `${plural(days, "день", "дня", "дней")} назад`;
}
