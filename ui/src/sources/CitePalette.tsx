"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useI18n } from "../i18n/index.js";
import type { VaultClient } from "../vault-client.js";
import type { LibrarySummary, Source, SourceHits } from "../wasm-core/typsidian_core.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { useMenuPosition, type CaretAnchor } from "../editor/InsertMenu.js";
import { authorsLine } from "./describe.js";

/** Строк в палитре (contracts/frontend-sources.md § Палитра). */
const PALETTE_LIMIT = 50;

export interface CitePaletteProps {
  anchor: CaretAnchor | null;
  client: VaultClient;
  summary: LibrarySummary | null;
  /** Растёт при изменении библиотеки — повод повторить поиск. */
  revision: number;
  readOnly: boolean;
  onPick: (source: Source) => void;
  onClose: () => void;
  onCreateFile: () => void;
  onRegisterFile: (path: string) => void;
}

/**
 * Палитра «Вставить цитату» у каретки (фича 017, макет 1b и 1c): поиск по
 * автору, названию, году и ключу. Ничего не вставляет сама — объявляет
 * выбор наверх; закрывается по Escape, нажатию мимо и после выбора.
 */
export function CitePalette({
  anchor,
  client,
  summary,
  revision,
  readOnly,
  onPick,
  onClose,
  onCreateFile,
  onRegisterFile,
}: CitePaletteProps): React.JSX.Element {
  const { t } = useI18n();
  const ref = useRef<HTMLDivElement | null>(null);
  const input = useRef<HTMLInputElement | null>(null);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SourceHits | null>(null);
  const [active, setActive] = useState(0);
  const [picking, setPicking] = useState(false);
  const asked = useRef(0);
  useDismissOnEscape(onClose);
  const position = useMenuPosition(ref, anchor);
  // До расчёта места палитра скрыта, а скрытое поле фокус не принимает —
  // фокус ставится, когда она показана.
  const placed = position !== null;
  useEffect(() => {
    if (placed) input.current?.focus();
  }, [placed]);

  useEffect(() => {
    const ticket = ++asked.current;
    void client.sources.search(query, PALETTE_LIMIT, false).then((result) => {
      // Ответ на устаревший запрос отбрасывается.
      if (ticket === asked.current) {
        setHits(result);
        setActive(0);
      }
    });
  }, [client, query, revision]);

  useEffect(() => {
    const onPointerDown = (event: PointerEvent): void => {
      const node = ref.current;
      if (node && event.target instanceof Node && !node.contains(event.target)) onClose();
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [onClose]);

  // Один ключ в двух файлах — под ключом имя файла (Edge Cases).
  const duplicated = useMemo(() => {
    const seen = new Map<string, number>();
    for (const hit of hits?.hits ?? []) seen.set(hit.source.key, (seen.get(hit.source.key) ?? 0) + 1);
    return seen;
  }, [hits]);

  const noFiles = summary !== null && summary.files.length === 0;
  const noEntries = summary !== null && summary.files.length > 0 && summary.source_count === 0;
  const rows = hits?.hits ?? [];

  const choose = (index: number): void => {
    const hit = rows[index];
    if (hit !== undefined) onPick(hit.source);
  };

  return (
    <div
      ref={ref}
      className="ts-cite"
      role="dialog"
      aria-label={t("sources.palette.label")}
      style={{
        left: position?.left ?? 0,
        top: position?.top ?? 0,
        visibility: position === null ? "hidden" : "visible",
      }}
      onKeyDown={(event) => {
        if (event.key === "ArrowDown") {
          event.preventDefault();
          setActive((value) => Math.min(rows.length - 1, value + 1));
        } else if (event.key === "ArrowUp") {
          event.preventDefault();
          setActive((value) => Math.max(0, value - 1));
        } else if (event.key === "Enter") {
          event.preventDefault();
          choose(active);
        }
      }}
    >
      {noFiles ? (
        <div className="ts-cite__empty">
          <strong>{t("sources.empty.title")}</strong>
          {readOnly ? (
            <p>{t("sources.empty.readOnly")}</p>
          ) : (
            <button type="button" className="ts-btn ts-btn--accent" onClick={onCreateFile}>
              {t("sources.empty.create")}
            </button>
          )}
          {summary.yaml_candidates.length > 0 && !readOnly ? (
            <>
              <button type="button" className="ts-btn" onClick={() => setPicking((open) => !open)}>
                {t("sources.empty.pick")}
              </button>
              {picking && (
                <ul className="ts-cite__candidates">
                  {summary.yaml_candidates.map((path) => (
                    <li key={path}>
                      <button type="button" className="ts-cite__candidate" onClick={() => onRegisterFile(path)}>
                        {path}
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </>
          ) : (
            <p className="ts-cite__hint">{t("sources.empty.hint")}</p>
          )}
        </div>
      ) : (
        <>
          <input
            ref={input}
            type="text"
            className="ts-cite__query"
            value={query}
            placeholder={t("sources.palette.placeholder")}
            aria-label={t("sources.palette.label")}
            spellCheck={false}
            onChange={(event) => setQuery(event.target.value)}
          />
          {noEntries && (
            <p className="ts-cite__hint">
              {t("sources.empty.noEntries", {
                path: summary.files.map((file) => file.path).join(", "),
              })}
            </p>
          )}
          <ul className="ts-cite__list" role="listbox" aria-label={t("sources.palette.label")}>
            {rows.map((hit, index) => (
              <li
                key={`${hit.source.file}\u0000${hit.source.key}`}
                role="option"
                aria-selected={index === active}
                className={index === active ? "ts-cite__row ts-cite__row--on" : "ts-cite__row"}
                onPointerMove={() => setActive(index)}
                onPointerDown={(event) => {
                  event.preventDefault();
                  choose(index);
                }}
              >
                <span className="ts-cite__main">
                  <span className="ts-cite__authors">{authorsLine(hit.source)}</span>
                  <span className="ts-cite__title">{hit.source.title}</span>
                </span>
                <span className="ts-cite__key">
                  {hit.source.key}
                  {(duplicated.get(hit.source.key) ?? 0) > 1 && (
                    <span className="ts-cite__file">{hit.source.file}</span>
                  )}
                </span>
              </li>
            ))}
          </ul>
          {hits !== null && rows.length === 0 && !noEntries && (
            <p className="ts-cite__hint">{t("sources.nothingFound")}</p>
          )}
          <div className="ts-cite__foot">
            <span>{t("sources.palette.footer")}</span>
            {hits !== null && hits.total_matches > rows.length && (
              <span>{t("sources.shown", { shown: rows.length, total: hits.total_matches })}</span>
            )}
          </div>
        </>
      )}
    </div>
  );
}
