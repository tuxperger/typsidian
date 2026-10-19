"use client";

import { memo, useRef, useState } from "react";
import type { OutlineRow as Row } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";

// Строка пункта оглавления (фича 015, макет «Typsidian Оглавление» 1a–1c).
//
// Мемоизирована: смена подсветки меняет `active` у двух строк, и только они
// перерисовываются (FR-007, SC-004). Ответ ядра на вопрос о текущем разделе
// строк не пересоздаёт, поэтому остальные пропы те же.

/** Сколько держать палец, чтобы увидеть текст строки целиком. */
const HOLD_MS = 500;

export interface OutlineRowProps {
  row: Row;
  active: boolean;
  /** Строка — та, на которую попадает Tab (roving tabindex). */
  focusable: boolean;
  onJump: (row: Row) => void;
  onToggle: (key: string) => void;
  onFocusRow: (key: string) => void;
}

export const OutlineRow = memo(function OutlineRow({
  row,
  active,
  focusable,
  onJump,
  onToggle,
  onFocusRow,
}: OutlineRowProps): React.JSX.Element {
  const { t } = useI18n();
  const heading = row.kind === "Heading";
  const text = row.segments.map((segment) => segment.text).join("");
  const prefix =
    row.kind === "Figure"
      ? t("outline.prefix.figure")
      : row.kind === "Table"
        ? t("outline.prefix.table")
        : null;
  const mono = !heading || row.text_is_source;
  // Удержание пальцем показывает обрезанный текст целиком: `title` на
  // сенсорном экране не всплывает (FR-001).
  const [whole, setWhole] = useState(false);
  const hold = useRef<number | null>(null);
  const holdDone = useRef(false);
  const cancelHold = (): void => {
    if (hold.current !== null) window.clearTimeout(hold.current);
    hold.current = null;
  };

  const classes = ["ts-outline__row"];
  if (whole) classes.push("ts-outline__row--whole");
  if (active) classes.push("ts-outline__row--active");
  if (row.dimmed) classes.push("ts-outline__row--dimmed");
  if (mono) classes.push("ts-outline__row--mono");

  return (
    <li
      role="treeitem"
      className={classes.join(" ")}
      data-key={row.key}
      aria-level={row.level}
      aria-selected={active}
      {...(active ? { "aria-current": "true" as const } : {})}
      {...(row.has_children ? { "aria-expanded": !row.collapsed } : {})}
      tabIndex={focusable ? 0 : -1}
      title={text}
      style={{ "--ts-outline-level": row.level } as React.CSSProperties}
      onFocus={() => onFocusRow(row.key)}
      onPointerDown={(event) => {
        if (event.pointerType !== "touch") return;
        holdDone.current = false;
        cancelHold();
        hold.current = window.setTimeout(() => {
          holdDone.current = true;
          setWhole((shown) => !shown);
        }, HOLD_MS);
      }}
      onPointerUp={cancelHold}
      onPointerLeave={cancelHold}
      onPointerCancel={cancelHold}
      onContextMenu={(event) => {
        // Долгое нажатие на телефоне вызывает ещё и системное меню.
        if (holdDone.current) event.preventDefault();
      }}
      onClick={() => {
        // Отпущенное удержание — не переход.
        if (holdDone.current) {
          holdDone.current = false;
          return;
        }
        onJump(row);
      }}
    >
      {row.has_children ? (
        <button
          type="button"
          tabIndex={-1}
          className={
            row.collapsed ? "ts-outline__chevron ts-outline__chevron--closed" : "ts-outline__chevron"
          }
          aria-label={row.collapsed ? t("outline.row.expand") : t("outline.row.collapse")}
          onClick={(event) => {
            // Шеврон сворачивает, а не переходит.
            event.stopPropagation();
            onToggle(row.key);
          }}
        />
      ) : (
        <span className="ts-outline__chevron ts-outline__chevron--none" aria-hidden="true" />
      )}
      <span className="ts-outline__text">
        {prefix !== null && <span className="ts-outline__prefix">{prefix} </span>}
        {row.kind === "Label" ? (
          <span className="ts-outline__label">
            &lt;
            <Segments row={row} />
            &gt;
          </span>
        ) : (
          <Segments row={row} />
        )}
      </span>
      {row.label !== undefined && row.kind !== "Label" && (
        <span className="ts-outline__label ts-outline__label--side">&lt;{row.label}&gt;</span>
      )}
    </li>
  );
});

/** Текст строки кусками ядра: совпадение фильтра подсвечено (макет 1b). */
function Segments({ row }: { row: Row }): React.JSX.Element {
  return (
    <>
      {row.segments.map((segment, index) =>
        segment.highlighted ? (
          <mark key={index} className="ts-outline__match">
            {segment.text}
          </mark>
        ) : (
          <span key={index}>{segment.text}</span>
        ),
      )}
    </>
  );
}
