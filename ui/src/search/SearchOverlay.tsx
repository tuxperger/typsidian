"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import type {
  Condition,
  Note,
  OpenText,
  PropertyKeyInfo,
  ReplacePreview as Preview,
  SearchQuery,
  SearchResults,
  SelectionResult,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { Icon } from "../icons/Icons.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { useI18n, type Phrase, type Translate } from "../i18n/index.js";
import { conditionParts, valueText } from "../note-panel/properties/conditions.js";
import type { OpenNoteEditors } from "../note-panel/links/openNoteEditors.js";
import { ReplacePreview } from "./ReplacePreview.js";
import { useReplace, type ReplaceState } from "./useReplace.js";

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
  /** Условия по свойствам, с которыми поиск открыт (клик по тегу, фича 020). */
  initialConditions?: Condition[];
  /** Mod+Enter: вставить условия блоком выборки (макет B3). */
  onInsertSelection?: (conditions: Condition[]) => void;
  /**
   * Замена по vault (фича 027): правки открытых заметок идут через их
   * редакторы (порт 013), тексты редакторов — в предпросмотр.
   */
  editors?: OpenNoteEditors | null;
  openTexts?: () => readonly OpenText[];
  /** Открыть сразу в режиме замены (`Mod+Shift+H`). */
  initialReplace?: boolean;
  onClose: () => void;
}

const NO_OPEN_TEXTS = (): readonly OpenText[] => [];

/** Без порта редакторов замена выключена; заглушка только для типа хука. */
const NO_EDITORS: OpenNoteEditors = {
  textOf: () => null,
  applyEdit: () => ({ ok: false, error: "NoEditor" }),
};

/** Плоский список строк выдачи — по нему ходят стрелки. */
interface Row {
  path: string;
  name: string;
}

export function SearchOverlay({
  client,
  currentNote,
  onOpenNote,
  initialConditions = [],
  onInsertSelection,
  editors = null,
  openTexts = NO_OPEN_TEXTS,
  initialReplace = false,
  onClose,
}: SearchOverlayProps): React.JSX.Element {
  const [replaceOn, setReplaceOn] = useState(initialReplace && editors !== null);
  const [replacement, setReplacement] = useState("");
  const [text, setText] = useState("");
  const [regex, setRegex] = useState(false);
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [linksToCurrent, setLinksToCurrent] = useState(false);
  const [mode, setMode] = useState<"And" | "Or">("And");
  const [results, setResults] = useState<SearchResults | null>(null);
  const [error, setError] = useState<Phrase | null>(null);
  const [busy, setBusy] = useState(false);
  const [selected, setSelected] = useState(0);
  // Условия по свойствам (фича 020): фишки над результатами, все сразу.
  const [conditions, setConditions] = useState<Condition[]>(initialConditions);
  const [selection, setSelection] = useState<SelectionResult | null>(null);
  const [knownKeys, setKnownKeys] = useState<string[]>([]);
  const { lang } = useI18n();

  useDismissOnEscape(onClose);
  const { t } = useI18n();

  const inputRef = useRef<HTMLInputElement | null>(null);
  useEffect(() => inputRef.current?.focus(), []);

  const currentPath = currentNote?.path.toString() ?? null;
  // Чип живёт, только пока есть на что ссылаться: закрытая заметка сделала бы
  // фильтр беспредметным, а включённым он бы остался.
  const linksTo = linksToCurrent ? currentPath : null;

  // Монотонная версия запроса: ответ на устаревший текст не должен вытеснить
  // более новый — та же причина, что и в предпросмотре (FR-009 фичи 005).
  const revision = useRef(0);

  const query: SearchQuery = useMemo(
    () => ({
      text,
      regex,
      case_sensitive: caseSensitive,
      links_to: linksTo ?? undefined,
      ...(conditions.length > 0 ? { properties: conditions } : {}),
      mode,
    }),
    [text, regex, caseSensitive, linksTo, conditions, mode],
  );
  const replace = useReplace({
    client,
    editors: editors ?? NO_EDITORS,
    openTexts,
    query: { search: query, replacement },
    enabled: replaceOn && editors !== null && text.trim() !== "",
  });

  useEffect(() => {
    // Режим замены строит свою выдачу (фича 027): обычный поиск не нужен.
    if (replaceOn) {
      revision.current += 1;
      setResults(null);
      setSelection(null);
      setBusy(false);
      return;
    }
    if (!text.trim() && linksTo === null) {
      setResults(null);
      setError(null);
      if (conditions.length === 0) {
        setSelection(null);
        setBusy(false);
        return;
      }
      // Только условия по свойствам: отбор без текстового поиска (B3).
      const mineSelect = ++revision.current;
      setBusy(true);
      void client.properties
        .select({ conditions, columns: ["название", "изменена"], limit: 500, view: "Table" }) // i18n-ignore: имена столбцов ядра
        .then((found) => {
          if (mineSelect !== revision.current) return;
          setSelection(found);
          setSelected(0);
        })
        .catch((cause: unknown) => {
          if (mineSelect === revision.current) setError(() => reasonOf(cause));
        })
        .finally(() => {
          if (mineSelect === revision.current) setBusy(false);
        });
      return;
    }
    setSelection(null);

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
          setError(() => reasonOf(cause));
          setResults(null);
        } finally {
          if (mine === revision.current) setBusy(false);
        }
      })();
    }, SEARCH_DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [client, query, text, linksTo, conditions, replaceOn]);

  // Ключи свойств vault - чтобы `статус:` в строке понималось как свойство.
  useEffect(() => {
    let stale = false;
    void client.properties
      .keys()
      .then((keys: PropertyKeyInfo[]) => !stale && setKnownKeys(keys.map((info) => info.key)))
      .catch(() => undefined);
    return () => {
      stale = true;
    };
  }, [client]);

  /** Условия из набранной строки уходят в фишки; остаток остаётся текстом. */
  const takeConditions = async (source: string): Promise<boolean> => {
    const parsed = await client.properties.parseConditions(source, knownKeys);
    if (parsed.conditions.length === 0) return false;
    setConditions((current) => [...current, ...parsed.conditions]);
    setText(parsed.text);
    return true;
  };

  const rows = useMemo<Row[]>(
    () =>
      selection !== null
        ? selection.rows.map((row) => ({
            path: row.path,
            name: row.cells[0] === undefined ? row.path : valueText(lang, row.cells[0]),
          }))
        : (results?.groups ?? []).flatMap((group) =>
            group.hits.map((hit) => ({ path: hit.path, name: hit.name })),
          ),
    [results, selection, lang],
  );

  const open = (path: string) => {
    onOpenNote(path);
    onClose();
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    // Escape закрывает поиск откуда угодно, а не только из поля ввода, —
    // `useDismissOnEscape` слушает окно.
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey) && replaceOn) {
      // В режиме замены Mod+Enter — «Заменить отмеченные» (research.md §6).
      event.preventDefault();
      void replace.apply();
      return;
    }
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey) && conditions.length > 0) {
      event.preventDefault();
      onInsertSelection?.(conditions);
      return;
    }
    if (event.key === "Enter" && text.trim() !== "" && !event.nativeEvent.isComposing) {
      // `тег:проект` + Enter: условие становится фишкой, а не уходит в поиск.
      event.preventDefault();
      void takeConditions(text).then((taken) => {
        const row = rows[selected];
        if (!taken && row) open(row.path);
      });
      return;
    }
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
      <section className="ts-search" aria-label={t("search.title")}>
        {/* Строка запроса: сам запрос, его режимы и счётчик найденного. */}
        <div className="ts-search__query">
          <Icon name="search" size={16} />
          <input
            ref={inputRef}
            className="ts-search__input"
            value={text}
            placeholder={t("properties.search.hint")}
            onChange={(event) => {
              const next = event.target.value;
              setText(next);
              // Слово `ключ:значение`, законченное пробелом, ядро превращает в условие.
              if (next.endsWith(" ") && next.includes(":")) void takeConditions(next);
            }}
            aria-label={t("search.inputLabel")}
          />
          <button
            type="button"
            className="ts-chip"
            aria-pressed={regex}
            onClick={() => setRegex((on) => !on)}
            title={t("search.regexHint")}
          >
            regex
          </button>
          <button
            type="button"
            className="ts-chip"
            aria-pressed={caseSensitive}
            onClick={() => setCaseSensitive((on) => !on)}
            title={t("search.caseHint")}
          >
            Aa
          </button>
          {editors !== null && (
            <button
              type="button"
              className="ts-chip"
              aria-pressed={replaceOn}
              onClick={() => setReplaceOn((on) => !on)}
              title={t("replace.toggleHint")}
            >
              <Icon name="updown" size={11} />
              {t("replace.toggle")}
            </button>
          )}
          <span className="ts-search__sep" />
          <span className="ts-search__count">
            {replaceOn
              ? replaceCount(t, replace.preview, replace.busy)
              : countLabel(t, results, busy)}
          </span>
          <button
            type="button"
            className="ts-btn ts-btn--ghost"
            onClick={onClose}
            aria-label={t("search.close")}
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        {/* Поле замены (фича 027, макет R1). */}
        {replaceOn && (
          <div className="ts-replace__field">
            <Icon name="enter" size={14} />
            <input
              id="ts-replace-input"
              className="ts-search__input"
              value={replacement}
              placeholder={t("replace.placeholder")}
              onChange={(event) => setReplacement(event.target.value)}
              aria-label={t("replace.inputLabel")}
            />
            {regex && <span className="ts-search__hit-meta">{t("replace.groupsHint")}</span>}
          </div>
        )}

        {/* Условия по свойствам (фича 020, макет B3): фишки, все должны выполняться. */}
        {conditions.length > 0 && (
          <div className="ts-search-cond" role="group" aria-label={t("properties.search.conditions")}>
            {conditions.map((condition, index) => {
              const parts = conditionParts(t, lang, condition);
              return (
                <span key={index} className="ts-search-cond__chip">
                  <b className="ts-search-cond__name">{parts.name}</b>
                  <span>{parts.text}</span>
                  <button
                    type="button"
                    className="ts-search-cond__x"
                    aria-label={t("properties.cond.remove", { name: parts.name })}
                    onClick={() => setConditions((current) => current.filter((_, at) => at !== index))}
                  >
                    <Icon name="x" size={9} />
                  </button>
                </span>
              );
            })}
            {onInsertSelection !== undefined && (
              <button
                type="button"
                className="ts-btn ts-btn--quiet ts-search-cond__select"
                aria-label={t("properties.search.asSelectLabel")}
                onClick={() => onInsertSelection(conditions)}
              >
                {t("properties.search.asSelect")}
              </button>
            )}
          </div>
        )}

        {/* Фильтры. В макете их шесть видов; здесь только те, под которыми
            есть данные ядра, — ссылки. Тегов и полей заголовка модель vault не
            знает, и рисовать неработающий чип хуже, чем не рисовать. */}
        <div className="ts-search__filters">
          <span className="ts-eyebrow">{t("search.filters")}</span>
          <div className="ts-search__chips">
            <button
              type="button"
              className="ts-chip"
              aria-pressed={linksToCurrent}
              disabled={currentPath === null}
              onClick={() => setLinksToCurrent((on) => !on)}
              title={
                currentPath === null ? t("search.linksToDisabled") : t("search.linksToHint")
              }
            >
              <Icon name="link" size={11} />
              {t("search.linksTo", { name: currentNote?.name ?? t("search.openNote") })}
            </button>
          </div>
          <div className="ts-search__mode">
            <span className="ts-search__mode-label">{t("search.modeLabel")}</span>
            <div className="ts-seg" role="group" aria-label={t("search.modeGroup")}>
              <button
                type="button"
                className="ts-seg__item"
                aria-pressed={mode === "And"}
                onClick={() => setMode("And")}
              >
                {t("search.and")}
              </button>
              <button
                type="button"
                className="ts-seg__item"
                aria-pressed={mode === "Or"}
                onClick={() => setMode("Or")}
              >
                {t("search.or")}
              </button>
            </div>
          </div>
        </div>

        {/* Строка запроса словами — как её поняло ядро. */}
        <div className="ts-search__echo">
          <span className="ts-search__echo-label">{t("search.queryLabel")}</span>
          <span>
            {replaceOn && text.trim() !== ""
              ? t("replace.echo", { from: regex ? `/${text}/` : `«${text}»`, to: replacement })
              : echo(t, text, regex, caseSensitive, linksTo, mode)}
          </span>
        </div>

        <div className="ts-search__results">
          {replaceOn && <ReplaceStatus replace={replace} />}
          {replaceOn && replace.preview !== null && (
            <ReplacePreview
              preview={replace.preview}
              selection={replace.selection}
              onToggleMatch={replace.toggleMatch}
              onToggleNote={replace.toggleNote}
              onOpen={open}
            />
          )}

          {error !== null && (
            <p className="ts-search__error" role="alert">
              {error(t)}
            </p>
          )}

          {error === null && results !== null && rows.length === 0 && !busy && (
            <p className="ts-search__empty">{t("search.nothingFound")}</p>
          )}

          {error === null && selection !== null && (
            <div className="ts-search__group">
              <div className="ts-search__group-head">
                <Icon name="tag" size={12} />
                {t("properties.search.count", { count: selection.total })}
                {selection.truncated && (
                  <span className="ts-search__group-meta">
                    {t("properties.search.truncated", {
                      shown: selection.rows.length,
                      total: selection.total,
                    })}
                  </span>
                )}
              </div>
              {selection.rows.length === 0 && !busy && (
                <p className="ts-search__empty">{t("properties.search.empty")}</p>
              )}
              {selection.rows.map((row, index) => (
                <div
                  key={row.path}
                  className={
                    index === selected ? "ts-search__hit ts-search__hit--active" : "ts-search__hit"
                  }
                  role="button"
                  tabIndex={-1}
                  aria-selected={index === selected}
                  onClick={() => open(row.path)}
                >
                  <div className="ts-search__hit-head">
                    <Icon name="file" size={12} />
                    <span className="ts-search__hit-name">
                      {row.cells[0] === undefined ? row.path : valueText(lang, row.cells[0])}
                    </span>
                    <span className="ts-search__hit-meta">{row.path}</span>
                    {row.cells[1] !== undefined && (
                      <span className="ts-search__hit-meta">{valueText(lang, row.cells[1])}</span>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}

          {!replaceOn && error === null && results === null && selection === null && !busy && (
            <p className="ts-search__empty">{t("search.emptyHint")}</p>
          )}

          {results?.groups.map((group) => (
            <div key={group.folder || "/"} className="ts-search__group">
              <div className="ts-search__group-head">
                <Icon name="folder" size={12} />
                {group.folder || t("search.vaultRoot")}
                <span className="ts-search__group-meta">
                  {group.hits.length} · {t("search.matches", { count: group.matches })}
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
                        {hit.first_line > 0 && t("search.line", { line: hit.first_line })}
                        {t("search.matches", { count: hit.matches })}
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
              {t("search.truncated", { count: results.total_notes })}
            </p>
          )}
        </div>

        {replaceOn ? (
          <div className="ts-search__hints ts-replace__foot">
            <span>{t("replace.hintToggle")}</span>
            <span>{t("replace.hintNote")}</span>
            <span>{t("replace.hintApply")}</span>
            <span className="ts-spacer" />
            {replace.preview !== null && (
              <span className="ts-search__count">
                {t("replace.selectedCount", {
                  selected: replace.selectedCount,
                  total: replace.preview.total_matches,
                  notes: replace.selectedNotes,
                })}
              </span>
            )}
            <button
              type="button"
              className="ts-btn ts-btn--accent"
              disabled={
                replace.preview === null ||
                replace.selectedCount === 0 ||
                replace.phase.kind === "running"
              }
              onClick={() => void replace.apply()}
            >
              {t("replace.apply", { count: replace.selectedCount })}
            </button>
          </div>
        ) : (
          <div className="ts-search__hints">
            <span>
              <Icon name="enter" size={10} /> {t("search.open")}
            </span>
            <span>{t("search.navigate")}</span>
            <span className="ts-spacer" />
            <span>{t("search.escClose")}</span>
          </div>
        )}
      </section>
    </div>
  );
}

/** Счётчик в шапке режима замены. */
function replaceCount(t: Translate, preview: Preview | null, busy: boolean): string {
  if (busy) return t("search.busy");
  if (preview === null) return "";
  return `${t("search.matches", { count: preview.total_matches })} · ${t("search.notes", {
    count: preview.total_notes,
  })}`;
}

/** Ход, итог и отказы замены (макет R2, R3). */
function ReplaceStatus({ replace }: { replace: ReplaceState }): React.JSX.Element | null {
  const { t } = useI18n();
  const nothing =
    replace.preview !== null && replace.preview.total_matches === 0 && !replace.busy ? (
      <p className="ts-search__empty">{t("replace.nothing")}</p>
    ) : null;
  const panel = <ReplacePhasePanel replace={replace} />;
  return (
    <>
      {panel}
      {nothing}
    </>
  );
}

function ReplacePhasePanel({ replace }: { replace: ReplaceState }): React.JSX.Element | null {
  const { t } = useI18n();
  const { phase } = replace;
  if (replace.error !== null) {
    return (
      <div className="ts-replace__panel ts-replace__panel--err" role="alert">
        <span className="ts-replace__title">{t("replace.badPatternTitle")}</span>
        <span className="ts-replace__meta">{replace.error(t)}</span>
      </div>
    );
  }
  if (replace.tooMany !== null) {
    return (
      <div className="ts-replace__panel ts-replace__panel--warn" role="status">
        <span className="ts-replace__title">{t("replace.tooMany")}</span>
        <span className="ts-replace__meta">
          {t("replace.tooManyDetail", { count: replace.tooMany })}
        </span>
      </div>
    );
  }
  if (phase.kind === "running") {
    return (
      <div className="ts-replace__panel ts-replace__panel--live" role="status">
        <div className="ts-replace__row">
          <span className="ts-replace__title">{t("replace.running")}</span>
          <span className="ts-spacer" />
          <button type="button" className="ts-btn" onClick={replace.stop}>
            {t("replace.stop")}
          </button>
        </div>
        <progress className="ts-replace__bar" max={phase.total} value={phase.done} />
        <span className="ts-replace__meta">
          {t("replace.progress", { done: phase.done, total: phase.total })}
        </span>
      </div>
    );
  }
  if (phase.kind === "finished") {
    const { report } = phase;
    if (report.readOnly) {
      return (
        <div className="ts-replace__panel ts-replace__panel--err" role="alert">
          <span className="ts-replace__title">{t("replace.readOnly")}</span>
        </div>
      );
    }
    const skipped: [string, string[]][] = [
      ["replace.skippedStale", report.stale],
      ["replace.skippedUnsaved", report.unsaved],
      ["replace.skippedFailed", report.failed],
    ];
    return (
      <div className="ts-replace__panel" role="status">
        <div className="ts-replace__row">
          <span className="ts-replace__title">
            {report.stopped !== null
              ? t("replace.stopped", report.stopped)
              : t("replace.done", { matches: report.matches, notes: report.notes.length })}
          </span>
          <span className="ts-spacer" />
          {replace.canUndo && (
            <button type="button" className="ts-btn" onClick={() => void replace.undo()}>
              {t("replace.undo")}
            </button>
          )}
        </div>
        {report.notes.length > 0 && (
          <span className="ts-replace__meta">{report.notes.join(" · ")}</span>
        )}
        {skipped.map(([key, names]) =>
          names.length === 0 ? null : (
            <span key={key} className="ts-replace__meta ts-replace__meta--err">
              {key === "replace.skippedStale"
                ? t("replace.skippedStale", { names: names.join(", ") })
                : key === "replace.skippedUnsaved"
                  ? t("replace.skippedUnsaved", { names: names.join(", ") })
                  : t("replace.skippedFailed", { names: names.join(", ") })}
              {key === "replace.skippedStale" && (
                <>
                  {" · "}
                  <button type="button" className="ts-replace__link" onClick={replace.refresh}>
                    {t("replace.refresh")}
                  </button>
                </>
              )}
            </span>
          ),
        )}
      </div>
    );
  }
  if (phase.kind === "undone") {
    const { report } = phase;
    return report.changed.length === 0 ? (
      <div className="ts-replace__panel" role="status">
        <span className="ts-replace__title">{t("replace.undone", { notes: report.restored })}</span>
      </div>
    ) : (
      <div className="ts-replace__panel ts-replace__panel--warn" role="status">
        <span className="ts-replace__title">{t("replace.undoPartial")}</span>
        <span className="ts-replace__meta">
          {t("replace.undone", { notes: report.restored })}.{" "}
          {t("replace.undoChanged", { names: report.changed.join(", ") })}
        </span>
      </div>
    );
  }
  return null;
}

/** Счётчик в шапке: «14 совпадений · 6 заметок» из макета. */
function countLabel(t: Translate, results: SearchResults | null, busy: boolean): string {
  if (busy) return t("search.busy");
  if (results === null) return "";
  return `${t("search.matches", { count: results.total_matches })} · ${t("search.notes", {
    count: results.total_notes,
  })}`;
}

/** Запрос словами — чтобы было видно, что именно ищется. */
function echo(
  t: Translate,
  text: string,
  regex: boolean,
  caseSensitive: boolean,
  linksTo: string | null,
  mode: "And" | "Or",
): string {
  const parts: string[] = [];
  if (text.trim()) parts.push(regex ? `/${text}/` : `«${text}»`);
  if (caseSensitive) parts.push(t("search.echoCase"));
  if (linksTo !== null) parts.push(t("search.linksTo", { name: linksTo }));
  if (parts.length === 0) return t("search.echoNone");
  return parts.join(` ${mode === "And" ? t("search.and") : t("search.or")} `);
}

/**
 * Отказ поиска. Неверный шаблон ядро описывает текстом движка регулярных
 * выражений — он показывается как есть, под переведённой подводкой (как
 * сообщения Typst, 010 FR-004). Чужие ошибки — как есть.
 */
function reasonOf(cause: unknown): Phrase {
  if (typeof cause === "object" && cause !== null && "message" in cause) {
    const detail = String((cause as { message: unknown }).message);
    const kind = "kind" in cause ? (cause as { kind: unknown }).kind : null;
    return kind === "BadPattern" ? (t) => t("search.badPattern", { detail }) : () => detail;
  }
  return () => String(cause);
}
