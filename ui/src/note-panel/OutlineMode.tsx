"use client";

import { memo, useCallback, useEffect, useRef, useState } from "react";
import type { OutlineKinds, OutlineRow as Row } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import { useContextMenu } from "../menu/ContextMenu.js";
import { OutlineRow } from "./OutlineRow.js";
import type { OutlineHandle } from "./useOutline.js";

// Режим «Структура» колонки заметки (фича 015, макет «Typsidian Оглавление»
// 1a–1c, contracts/frontend-outline.md § OutlineMode). Всё здесь — разметка
// ответа ядра: какие строки видны, какая подсвечена, что совпало с фильтром,
// решило ядро; режим только рисует и переводит клавиши в действия.

/** Что открыто в редакторе — от этого зависит, есть ли оглавление вообще (FR-012). */
export type OutlineTarget = "none" | "board" | "tooLarge" | "note";

export interface OutlineModeProps {
  target: OutlineTarget;
  outline: OutlineHandle;
  kinds: OutlineKinds;
  onKinds: (kinds: OutlineKinds) => void;
  /** Перейти к пункту: каретка, предпросмотр, закрытие шторки (FR-003, FR-004). */
  onJump: (row: Row) => void;
  /** Esc вне фильтра и на пустом фильтре — фокус обратно в редактор. */
  onFocusEditor: () => void;
  /** Поставить фокус в поле фильтра при появлении — у шторок (макет 1b). */
  focusFilter?: boolean;
}

// Мемоизирован: каждое нажатие в редакторе перерисовывает рабочую область, а
// дерево в 2 000 строк меняется только с ответом ядра (SC-004).
export const OutlineMode = memo(function OutlineMode({
  target,
  outline,
  kinds,
  onKinds,
  onJump,
  onFocusEditor,
  focusFilter = false,
}: OutlineModeProps): React.JSX.Element {
  const { t } = useI18n();
  const menu = useContextMenu();
  const { view, active, filter, setFilter, toggle } = outline;
  const rows = view?.rows ?? [];
  const treeRef = useRef<HTMLUListElement | null>(null);
  const filterRef = useRef<HTMLInputElement | null>(null);
  // Строка, на которую попадает Tab: последняя, где был фокус; без неё —
  // подсвеченная, без подсветки — первая.
  const [focusKey, setFocusKey] = useState<string | null>(null);
  const tabKey =
    focusKey !== null && rows.some((row) => row.key === focusKey)
      ? focusKey
      : (active ?? rows[0]?.key ?? null);

  useEffect(() => {
    if (focusFilter) filterRef.current?.focus();
  }, [focusFilter]);

  const rowElement = useCallback((key: string): HTMLElement | null => {
    const tree = treeRef.current;
    if (tree === null) return null;
    for (const node of tree.querySelectorAll<HTMLElement>("[data-key]")) {
      if (node.dataset.key === key) return node;
    }
    return null;
  }, []);

  // Подсветка сменилась — строка должна быть видна в колонке (FR-007).
  useEffect(() => {
    if (active !== null) rowElement(active)?.scrollIntoView({ block: "nearest" });
  }, [active, rowElement]);

  const focusRow = useCallback(
    (key: string | undefined) => {
      if (key === undefined) return;
      setFocusKey(key);
      rowElement(key)?.focus();
    },
    [rowElement],
  );

  const openKindsMenu = (button: HTMLElement): void => {
    // Меню открывается под кнопкой, а не в точке щелчка: с клавиатуры точки нет.
    const rect = button.getBoundingClientRect();
    const at = new MouseEvent("contextmenu", { clientX: rect.left, clientY: rect.bottom });
    menu.open(at, [
      {
        label: t("outline.kinds.figures"),
        checked: kinds.figures ?? false,
        run: () => onKinds({ ...kinds, figures: !(kinds.figures ?? false) }),
      },
      {
        label: t("outline.kinds.labels"),
        checked: kinds.labels ?? false,
        run: () => onKinds({ ...kinds, labels: !(kinds.labels ?? false) }),
      },
    ]);
  };

  const kindsButton = (
    <button
      type="button"
      className="ts-outline__kinds"
      aria-haspopup="menu"
      aria-label={t("outline.kinds.menu")}
      title={t("outline.kinds.menu")}
      onClick={(event) => openKindsMenu(event.currentTarget)}
    />
  );

  if (target !== "note") {
    const phrase =
      target === "board"
        ? t("outline.empty.board")
        : target === "tooLarge"
          ? t("outline.empty.tooLarge")
          : t("outline.empty.openNote");
    return <p className="ts-outline__empty">{phrase}</p>;
  }
  if (view === null) {
    return (
      <p className="ts-outline__empty" role="status">
        {t("outline.loading")}
      </p>
    );
  }
  if (view.state === "NoHeadings" && filter === "") {
    return (
      <div className="ts-outline">
        <div className="ts-outline__bar ts-outline__bar--end">{kindsButton}</div>
        <p className="ts-outline__empty">{t("outline.empty.noHeadings")}</p>
      </div>
    );
  }

  const onTreeKey = (event: React.KeyboardEvent<HTMLUListElement>): void => {
    const index = rows.findIndex((row) => row.key === tabKey);
    const row = rows[index];
    const move = (next: number): void => {
      event.preventDefault();
      focusRow(rows[Math.max(0, Math.min(rows.length - 1, next))]?.key);
    };
    switch (event.key) {
      case "ArrowDown":
        move(index + 1);
        break;
      case "ArrowUp":
        move(index - 1);
        break;
      case "Home":
        move(0);
        break;
      case "End":
        move(rows.length - 1);
        break;
      case "ArrowLeft":
        if (row === undefined) break;
        event.preventDefault();
        if (row.has_children && !row.collapsed) toggle(row.key);
        else if (row.parent !== undefined) focusRow(row.parent);
        break;
      case "ArrowRight":
        if (row === undefined || !row.has_children) break;
        event.preventDefault();
        if (row.collapsed) toggle(row.key);
        else focusRow(rows[index + 1]?.key);
        break;
      case "Enter":
      case " ":
        if (row === undefined) break;
        event.preventDefault();
        onJump(row);
        break;
      case "Escape":
        event.preventDefault();
        onFocusEditor();
        break;
    }
  };

  const onFilterKey = (event: React.KeyboardEvent<HTMLInputElement>): void => {
    switch (event.key) {
      case "Escape":
        event.preventDefault();
        if (filter !== "") setFilter("");
        else onFocusEditor();
        break;
      case "ArrowDown":
        event.preventDefault();
        focusRow(rows[0]?.key);
        break;
      case "Enter": {
        event.preventDefault();
        const first = rows.find((candidate) => !candidate.dimmed);
        if (first !== undefined) onJump(first);
        break;
      }
    }
  };

  return (
    <div className="ts-outline">
      <div className="ts-outline__bar">
        <input
          ref={filterRef}
          type="search"
          className="ts-outline__filter"
          value={filter}
          placeholder={t("outline.filter.placeholder")}
          aria-label={t("outline.filter.label")}
          onChange={(event) => setFilter(event.target.value)}
          onKeyDown={onFilterKey}
        />
        {kindsButton}
      </div>
      {view.state === "NoMatches" ? (
        <p className="ts-outline__empty" role="status">
          {t("outline.empty.noMatches")}
        </p>
      ) : (
        <ul
          ref={treeRef}
          role="tree"
          className="ts-outline__tree"
          aria-label={t("outline.treeLabel")}
          onKeyDown={onTreeKey}
        >
          {rows.map((row) => (
            <OutlineRow
              key={row.key}
              row={row}
              active={row.key === active}
              focusable={row.key === tabKey}
              onJump={onJump}
              onToggle={toggle}
              onFocusRow={setFocusKey}
            />
          ))}
        </ul>
      )}
    </div>
  );
});
