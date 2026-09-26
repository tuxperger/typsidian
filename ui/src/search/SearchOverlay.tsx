"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import type {
  Note,
  SearchQuery,
  SearchResults,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { Icon } from "../icons/Icons.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

// Экран расширенного поиска (макет «Typsidian Утилиты» § Поиск).
//
// Принцип I: где совпало, сколько совпало и как сгруппировать — решает ядро.
// Здесь только ритм запросов, выделение строки и клавиши.

/** Пауза после последнего нажатия, по истечении которой уходит запрос. */
export const SEARCH_DEBOUNCE_MS = 200;

export interface SearchOverlayProps {
  client: VaultClient;
  /** Заметка, открытая в редакторе: она предлагается чипу «ссылается на …». */
  currentNote: Note | null;
  onOpenNote: (path: string) => void;
  onClose: () => void;
}

/** Плоский список строк выдачи — по нему ходят стрелки. */
interface Row {
  path: string;
  name: string;
}

export function SearchOverlay({
  client,
  currentNote,
  onOpenNote,
  onClose,
}: SearchOverlayProps): React.JSX.Element {
  const [text, setText] = useState("");
  const [regex, setRegex] = useState(false);
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [linksToCurrent, setLinksToCurrent] = useState(false);
  const [mode, setMode] = useState<"And" | "Or">("And");
  const [results, setResults] = useState<SearchResults | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [selected, setSelected] = useState(0);

  useDismissOnEscape(onClose);

  const inputRef = useRef<HTMLInputElement | null>(null);
  useEffect(() => inputRef.current?.focus(), []);

  const currentPath = currentNote?.path.toString() ?? null;
  // Чип живёт, только пока есть на что ссылаться: закрытая заметка сделала бы
  // фильтр беспредметным, а включённым он бы остался.
  const linksTo = linksToCurrent ? currentPath : null;

  // Монотонная версия запроса: ответ на устаревший текст не должен вытеснить
  // более новый — та же причина, что и в предпросмотре (FR-009 фичи 005).
  const revision = useRef(0);

  useEffect(() => {
    const query: SearchQuery = {
      text,
      regex,
      case_sensitive: caseSensitive,
      links_to: linksTo ?? undefined,
      mode,
    };
    if (!text.trim() && linksTo === null) {
      setResults(null);
      setError(null);
      setBusy(false);
      return;
    }

    const mine = ++revision.current;
    setBusy(true);
    const timer = window.setTimeout(() => {
      void (async () => {
        try {
          const found = await client.search(query);
          if (mine !== revision.current) return;
          setResults(found);
          setError(null);
          setSelected(0);
        } catch (cause) {
          if (mine !== revision.current) return;
          // Ядро называет причину само — своего текста здесь нет: разойтись с
          // ним было бы легче, чем совпасть.
          setError(reasonOf(cause));
          setResults(null);
        } finally {
          if (mine === revision.current) setBusy(false);
        }
      })();
    }, SEARCH_DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [client, text, regex, caseSensitive, linksTo, mode]);

  const rows = useMemo<Row[]>(
    () =>
      (results?.groups ?? []).flatMap((group) =>
        group.hits.map((hit) => ({ path: hit.path, name: hit.name })),
      ),
    [results],
  );

  const open = (path: string) => {
    onOpenNote(path);
    onClose();
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    // Escape закрывает поиск откуда угодно, а не только из поля ввода, —
    // `useDismissOnEscape` слушает окно.
    if (rows.length === 0) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelected((index) => Math.min(rows.length - 1, index + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelected((index) => Math.max(0, index - 1));
    } else if (event.key === "Enter") {
      event.preventDefault();
      const row = rows[selected];
      if (row) open(row.path);
    }
  };

  let position = -1;

  return (
    <div className="ts-overlay" onKeyDown={onKeyDown} role="presentation">
      <section className="ts-search" aria-label="Поиск по vault">
        {/* Строка запроса: сам запрос, его режимы и счётчик найденного. */}
        <div className="ts-search__query">
          <Icon name="search" size={16} />
          <input
            ref={inputRef}
            className="ts-search__input"
            value={text}
            placeholder="искать по заметкам…"
            onChange={(event) => setText(event.target.value)}
            aria-label="Строка поиска"
          />
          <button
            type="button"
            className="ts-chip"
            aria-pressed={regex}
            onClick={() => setRegex((on) => !on)}
            title="Искать регулярным выражением"
          >
            regex
          </button>
          <button
            type="button"
            className="ts-chip"
            aria-pressed={caseSensitive}
            onClick={() => setCaseSensitive((on) => !on)}
            title="Различать регистр"
          >
            Aa
          </button>
          <span className="ts-search__sep" />
          <span className="ts-search__count">{countLabel(results, busy)}</span>
          <button
            type="button"
            className="ts-btn ts-btn--ghost"
            onClick={onClose}
            aria-label="Закрыть поиск"
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        {/* Фильтры. В макете их шесть видов; здесь только те, под которыми
            есть данные ядра, — ссылки. Тегов и полей заголовка модель vault не
            знает, и рисовать неработающий чип хуже, чем не рисовать. */}
        <div className="ts-search__filters">
          <span className="ts-eyebrow">фильтры</span>
          <div className="ts-search__chips">
            <button
              type="button"
              className="ts-chip"
              aria-pressed={linksToCurrent}
              disabled={currentPath === null}
              onClick={() => setLinksToCurrent((on) => !on)}
              title={
                currentPath === null
                  ? "Откройте заметку, чтобы искать ссылающиеся на неё"
                  : "Только заметки, ссылающиеся на открытую"
              }
            >
              <Icon name="link" size={11} />
              ссылается на {currentNote?.name ?? "открытую заметку"}
            </button>
          </div>
          <div className="ts-search__mode">
            <span className="ts-search__mode-label">И / ИЛИ</span>
            <div className="ts-seg" role="group" aria-label="Как складывать условия">
              <button
                type="button"
                className="ts-seg__item"
                aria-pressed={mode === "And"}
                onClick={() => setMode("And")}
              >
                И
              </button>
              <button
                type="button"
                className="ts-seg__item"
                aria-pressed={mode === "Or"}
                onClick={() => setMode("Or")}
              >
                ИЛИ
              </button>
            </div>
          </div>
        </div>

        {/* Строка запроса словами — как её поняло ядро. */}
        <div className="ts-search__echo">
          <span className="ts-search__echo-label">запрос:</span>
          <span>{echo(text, regex, caseSensitive, linksTo, mode)}</span>
        </div>

        <div className="ts-search__results">
          {error !== null && (
            <p className="ts-search__error" role="alert">
              {error}
            </p>
          )}

          {error === null && results !== null && rows.length === 0 && !busy && (
            <p className="ts-search__empty">ничего не найдено</p>
          )}

          {error === null && results === null && !busy && (
            <p className="ts-search__empty">
              Введите строку или включите фильтр — поиск идёт по всем заметкам vault.
            </p>
          )}

          {results?.groups.map((group) => (
            <div key={group.folder || "/"} className="ts-search__group">
              <div className="ts-search__group-head">
                <Icon name="folder" size={12} />
                {group.folder || "корень vault"}
                <span className="ts-search__group-meta">
                  {group.hits.length} · {group.matches} совпадений
                </span>
              </div>

              {group.hits.map((hit) => {
                position += 1;
                const active = position === selected;
                return (
                  <div
                    key={hit.path}
                    className={active ? "ts-search__hit ts-search__hit--active" : "ts-search__hit"}
                    role="button"
                    tabIndex={-1}
                    aria-selected={active}
                    onClick={() => open(hit.path)}
                  >
                    <div className="ts-search__hit-head">
                      <Icon name="file" size={12} />
                      <span className="ts-search__hit-name">{hit.name}</span>
                      <span className="ts-search__hit-meta">
                        {hit.first_line > 0 && `стр. ${hit.first_line} · `}
                        {hit.matches} совпадений
                      </span>
                    </div>
                    {hit.lines.length > 0 && (
                      <div className="ts-search__context">
                        {hit.lines.map((line) => (
                          <div
                            key={line.number}
                            className={
                              line.is_match
                                ? "ts-search__line ts-search__line--match"
                                : "ts-search__line"
                            }
                          >
                            {line.segments.map((part, index) =>
                              part.highlighted ? (
                                <mark key={index} className="ts-search__mark">
                                  {part.text}
                                </mark>
                              ) : (
                                <span key={index}>{part.text}</span>
                              ),
                            )}
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          ))}

          {results?.truncated && (
            <p className="ts-search__empty">
              Показаны первые {results.total_notes} заметок — уточните запрос.
            </p>
          )}
        </div>

        <div className="ts-search__hints">
          <span>
            <Icon name="enter" size={10} /> открыть
          </span>
          <span>↑↓ выбор</span>
          <span className="ts-spacer" />
          <span>Esc закрыть</span>
        </div>
      </section>
    </div>
  );
}

/** Счётчик в шапке: «14 совпадений · 6 заметок» из макета. */
function countLabel(results: SearchResults | null, busy: boolean): string {
  if (busy) return "ищем…";
  if (results === null) return "";
  return `${results.total_matches} совпадений · ${results.total_notes} заметок`;
}

/** Запрос словами — чтобы было видно, что именно ищется. */
function echo(
  text: string,
  regex: boolean,
  caseSensitive: boolean,
  linksTo: string | null,
  mode: "And" | "Or",
): string {
  const parts: string[] = [];
  if (text.trim()) parts.push(regex ? `/${text}/` : `«${text}»`);
  if (caseSensitive) parts.push("с учётом регистра");
  if (linksTo !== null) parts.push(`ссылается на ${linksTo}`);
  if (parts.length === 0) return "условий нет";
  return parts.join(mode === "And" ? " И " : " ИЛИ ");
}

/** Текст отказа от ядра; чужие ошибки — как есть. */
function reasonOf(cause: unknown): string {
  if (typeof cause === "object" && cause !== null && "message" in cause) {
    return String((cause as { message: unknown }).message);
  }
  return String(cause);
}
