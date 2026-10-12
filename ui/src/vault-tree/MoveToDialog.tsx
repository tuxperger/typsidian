"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import type { VaultClient } from "../vault-client.js";
import type { MoveTarget } from "../wasm-core/typsidian_core.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import type { MoveEntryRef } from "./useMoveEntry.js";

export interface MoveToDialogProps {
  client: VaultClient;
  entry: MoveEntryRef;
  onPick: (folder: string) => void;
  onClose: () => void;
}

/**
 * «Переместить в…» — перенос без перетаскивания (фича 023, FR-021…FR-023).
 *
 * Окно в ряду с палитрой команд: то же место, те же строки. Список папок и
 * фильтр даёт ядро (`moveTargets`) — здесь строки рисуются как пришли: какие
 * папки допустимы и что считается совпадением, интерфейс не решает
 * (принцип I).
 */
export function MoveToDialog({
  client,
  entry,
  onPick,
  onClose,
}: MoveToDialogProps): React.JSX.Element {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [targets, setTargets] = useState<MoveTarget[] | null>(null);
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLUListElement | null>(null);

  useDismissOnEscape(onClose);

  useEffect(() => {
    let cancelled = false;
    void client.moveTargets(entry.path, query).then(
      (found) => {
        if (cancelled) return;
        setTargets(found);
        setActive(0);
      },
      () => {
        if (!cancelled) setTargets([]);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [client, entry.path, query]);

  const rows = targets ?? [];

  const pick = useCallback(
    (target: MoveTarget | undefined) => {
      if (target === undefined) return;
      onClose();
      onPick(target.path);
    },
    [onClose, onPick],
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
        pick(rows[active]);
      }
    },
    [rows, active, pick],
  );

  useEffect(() => {
    const selected = listRef.current?.querySelector<HTMLElement>('[aria-selected="true"]');
    selected?.scrollIntoView({ block: "nearest" });
  }, [active]);

  return (
    <div
      className="ts-palette-scrim"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className="ts-palette"
        role="dialog"
        aria-modal="true"
        aria-label={t("tree.move.dialog.label", { name: entry.name })}
        onKeyDown={onKeyDown}
      >
        <div className="ts-palette__group ts-palette__group--move">
          {t("tree.move.dialog.title")} <b>{entry.name}</b>
        </div>
        <div className="ts-palette__search">
          <Icon name="folder" size={15} />
          {/* eslint-disable-next-line jsx-a11y/no-autofocus -- окно открыто
              ради выбора папки; фокус в поле — это и есть действие */}
          <input
            autoFocus
            type="text"
            role="combobox"
            aria-expanded="true"
            aria-controls="ts-move-list"
            aria-activedescendant={rows[active] ? rowId(active) : undefined}
            value={query}
            aria-label={t("tree.move.dialog.label", { name: entry.name })}
            placeholder={t("tree.move.dialog.placeholder")}
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>

        <ul className="ts-palette__list" id="ts-move-list" role="listbox" ref={listRef}>
          {rows.map((target, index) => (
            <li
              key={target.path}
              id={rowId(index)}
              role="option"
              aria-selected={index === active}
              className="ts-palette__row"
              onClick={() => pick(target)}
            >
              <Icon name="folder" size={14} className="ts-palette__icon" />
              <span className="ts-palette__title">
                <TargetName target={target} />
              </span>
              <span className="ts-palette__meta">
                {target.path === ""
                  ? t("tree.move.dialog.root")
                  : target.is_section && target.parent === ""
                    ? t("tree.section.badge")
                    : target.parent}
              </span>
            </li>
          ))}
          {targets !== null && rows.length === 0 && (
            <li className="ts-palette__empty" role="presentation">
              {t("tree.move.dialog.empty")}
            </li>
          )}
        </ul>

        <div className="ts-palette__hints">
          <span>{t("tree.move.dialog.hintSelect")}</span>
          <span>{t("tree.move.dialog.hintMove")}</span>
          <span>{t("tree.move.dialog.hintClose")}</span>
        </div>
      </div>
    </div>
  );
}

function rowId(index: number): string {
  return `ts-move-target-${index}`;
}

/** Имя папки с подсветкой совпадения: где совпало, сообщает ядро. */
function TargetName({ target }: { target: MoveTarget }): React.JSX.Element {
  const match = target.name_match;
  if (match === null || match === undefined) return <>{target.name}</>;
  const start = match.start;
  const end = match.end;
  return (
    <>
      {target.name.slice(0, start)}
      <mark className="ts-palette__match">{target.name.slice(start, end)}</mark>
      {target.name.slice(end)}
    </>
  );
}
