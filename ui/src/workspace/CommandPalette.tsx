"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Icon, type IconName } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import type { Note } from "../wasm-core/typsidian_core.js";
import { useDismissOnEscape } from "./useDismissOnEscape.js";

// Палитра команд ⌘K — артборд 3 нового дизайна (фича 044, `specs/044-…/design/Palette.dc.html`).
//
// Что здесь есть и чего нет: фильтрация, навигация с клавиатуры и вызов
// действия. Списка команд она не придумывает — он приходит сверху, из
// `Workspace`, где эти действия и живут. Иначе палитра стала бы вторым местом,
// знающим, что умеет приложение.

export interface PaletteCommand {
  id: string;
  title: string;
  /**
   * Название по-английски — палитра находит команду и по нему, на любом языке
   * интерфейса (фича 010, FR-024). Показывается всегда `title`.
   */
  titleEn?: string;
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
  | { kind: "note"; key: string; title: string; meta: string; match: number; note: Note }
  | { kind: "command"; key: string; title: string; meta: string; match: number; command: PaletteCommand };

export function CommandPalette({
  notes,
  commands,
  onOpenNote,
  onClose,
}: CommandPaletteProps): React.JSX.Element {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLUListElement | null>(null);

  useDismissOnEscape(onClose);

  const noteRows = useMemo<Row[]>(() => {
    const needle = query.trim().toLowerCase();
    return notes
      .filter((note) => note.name.toLowerCase().includes(needle))
      .map((note) => ({
        kind: "note" as const,
        key: `note:${note.path}`,
        title: note.name,
        meta: folderOf(note.path),
        match: needle === "" ? -1 : note.name.toLowerCase().indexOf(needle),
        note,
      }));
  }, [notes, query]);

  const commandRows = useMemo<Row[]>(() => {
    const needle = query.trim().toLowerCase();
    return commands
      .filter(
        (command) =>
          command.title.toLowerCase().includes(needle) ||
          (command.titleEn?.toLowerCase().includes(needle) ?? false),
      )
      .map((command) => ({
        kind: "command" as const,
        key: `command:${command.id}`,
        title: command.title,
        meta: command.meta ?? "",
        match: needle === "" ? -1 : command.title.toLowerCase().indexOf(needle),
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
      // Escape намеренно не здесь: он закрывает палитру из любого места —
      // этим занят `useDismissOnEscape`, общий для всех наложений.
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
        aria-label={t("palette.label")}
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
            className="ts-palette__input"
            type="text"
            role="combobox"
            aria-expanded="true"
            aria-controls="ts-palette-list"
            aria-activedescendant={rows[active] ? rowId(rows[active]) : undefined}
            value={query}
            aria-label={t("palette.inputLabel")}
            placeholder={t("palette.placeholder")}
            onChange={(event) => {
              setQuery(event.target.value);
              setActive(0);
            }}
          />
          {/* Клавиша закрытия — и подсказка, и кнопка: на сенсорном экране
              Esc нажать нечем. */}
          <button
            type="button"
            className="ts-kbd ts-palette__esc"
            aria-label={t("palette.close")}
            onClick={onClose}
          >
            {t("palette.escKey")}
          </button>
        </div>

        <ul className="ts-palette__list" id="ts-palette-list" role="listbox" ref={listRef}>
          {noteRows.length > 0 && (
            <li className="ts-palette__group" role="presentation">
              {t("palette.groupNotes")}
            </li>
          )}
          {noteRows.map((row) => (
            <PaletteRow
              key={row.key}
              row={row}
              needleLength={query.trim().length}
              selected={rows[active]?.key === row.key}
              onRun={() => runRow(row)}
            />
          ))}

          {commandRows.length > 0 && (
            <li className="ts-palette__group" role="presentation">
              {t("palette.groupCommands")}
            </li>
          )}
          {commandRows.map((row) => (
            <PaletteRow
              key={row.key}
              row={row}
              needleLength={query.trim().length}
              selected={rows[active]?.key === row.key}
              onRun={() => runRow(row)}
            />
          ))}

          {rows.length === 0 && (
            <li className="ts-palette__empty" role="presentation">
              {t("palette.nothingFound")}
            </li>
          )}
        </ul>

        <div className="ts-palette__hints">
          <span>{t("palette.hintNavigate")}</span>
          <span>{t("palette.hintOpen")}</span>
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
  needleLength,
  selected,
  onRun,
}: {
  row: Row;
  /** Длина запроса: совпадение выделяется от `row.match` на столько символов. */
  needleLength: number;
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
      <span className="ts-palette__title">
        <Highlighted text={row.title} at={row.match} length={needleLength} />
      </span>
      {row.meta && <span className="ts-palette__meta">{row.meta}</span>}
    </li>
  );
}

/** Совпадение с запросом — цветом акцента, как у «Переместить в…». */
function Highlighted({
  text,
  at,
  length,
}: {
  text: string;
  at: number;
  length: number;
}): React.JSX.Element {
  if (at < 0 || length === 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, at)}
      <mark className="ts-palette__match">{text.slice(at, at + length)}</mark>
      {text.slice(at + length)}
    </>
  );
}

/**
 * Папка заметки — где она лежит (артборд 3); у заметки в корне vault — пусто.
 */
function folderOf(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash <= 0 ? "" : `${path.slice(0, slash)}/`;
}
