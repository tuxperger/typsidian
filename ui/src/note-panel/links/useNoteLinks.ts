"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  LinkOutcome,
  MentionScan,
  MentionsAvailability,
  OpenText,
  SiteRow,
  SourceGroup,
} from "../../wasm-core/typsidian_core.js";
import type { LinksClient } from "../../vault-client.js";

// Ритм запросов режима «Связи» (фича 013, contracts/note-panel.md). Правил
// здесь нет: что считается ссылкой и упоминанием, порядок источников, фрагменты
// и сама запись ссылки — в ядре. Хук решает только, когда спросить ядро ещё
// раз и какой ответ уже устарел.

/** Размер порции обратных ссылок и упоминаний (FR-005). */
export const LINKS_PAGE = 50;

/** Пауза после события об изменении, прежде чем перезапросить страницу (SC-003). */
export const LINKS_REVISION_DEBOUNCE_MS = 150;

/**
 * Больше стольких названных заметок точечное обновление упоминаний дороже
 * прохода заново: тогда проход и запускается.
 */
const MENTION_POINT_LIMIT = 24;

/**
 * Что изменилось с прошлого обновления панели. `all` — неизвестно что
 * (переименование, массовое изменение): упоминания ищутся заново. Иначе
 * `paths` — затронутые заметки; пустой набор — менялись только ссылки,
 * упоминания прежние.
 *
 * Копит `Workspace`, а хук забирает порцию при каждом увеличении счётчика:
 * два события в одном кадре React схлопнул бы в одно состояние, и пути первого
 * потерялись бы.
 */
export interface LinksChanges {
  all: boolean;
  paths: Set<string>;
}

export interface NoteLinksInput {
  /** `null` — vault не открыт. */
  client: LinksClient | null;
  /** Путь открытой заметки или доски; `null` — ничего не открыто. */
  target: string | null;
  /**
   * Тексты заметок, открытых в редакторе. В 013 — пустой список: единственная
   * открытая заметка и есть цель. 014 передаёт тексты всех вкладок.
   */
  openTexts: readonly OpenText[];
  /** Скрытая колонка ничего не запрашивает (research.md §14). */
  enabled: boolean;
  /** Счётчик обновления (research.md §14): растёт на события vault, автосохранение и «Связать». */
  revision: number;
  /** Забрать накопленные изменения; вызывается при каждом изменении `revision`. */
  takeChanges: () => LinksChanges;
  /** Идёт массовое изменение: запросы откладываются до его конца. */
  bulkChanging: boolean;
  mentionsExpanded: boolean;
  /** «Связать» записало файл: Workspace увеличивает счётчик обновления. */
  onLinked: (sourcePath: string) => void;
}

export type BacklinksView =
  | { phase: "idle" }
  | { phase: "collecting" }
  | { phase: "loading" }
  | { phase: "failed" }
  | {
      phase: "ready";
      groups: SourceGroup[];
      /** Ссылающихся заметок по индексу — точное число. */
      totalSources: number;
      hasMore: boolean;
      loadingMore: boolean;
    };

export type MentionsView =
  | { phase: "collapsed" }
  | { phase: "unavailable"; reason: MentionsAvailability }
  | { phase: "scanning"; scanned: number; total: number; groups: SourceGroup[] }
  | { phase: "ready"; groups: SourceGroup[] }
  | { phase: "failed" };

/** Ключ строки-упоминания: заметка плюс место. */
export function mentionKey(path: string, row: SiteRow): string {
  return `${path}:${row.place.byte_start}`;
}

export interface NoteLinks {
  backlinks: BacklinksView;
  mentions: MentionsView;
  /** Название цели для образца ссылки в пустом состоянии; `null` — ещё не получено. */
  title: string | null;
  /**
   * Заметки, у которых «Связать» ответило `Stale`, — с группой, какой она была:
   * тег «устарело» показывается и тогда, когда после обновления упоминаний в
   * заметке не осталось. Живёт до смены цели или сворачивания списка.
   */
  staleSources: ReadonlyMap<string, SourceGroup>;
  /** Исход последней попытки «Связать» по строке (кроме удавшейся). */
  outcomes: ReadonlyMap<string, LinkOutcome>;
  /** Строка, запись которой идёт сейчас. */
  linkingKey: string | null;
  /** Число для счётчика раздела (research.md §13): ссылок, пока загружено всё, иначе заметок. */
  backlinkCount: { unit: "links" | "notes"; value: number } | null;
  /** Число упоминаний — только после завершённого прохода. */
  mentionCount: number | null;
  showMore: () => void;
  link: (source: SourceGroup, row: SiteRow) => Promise<void>;
}

interface BacklinksState {
  target: string;
  view: BacklinksView;
}

interface MentionsState {
  target: string;
  view: MentionsView;
}

function rowCount(groups: readonly SourceGroup[]): number {
  return groups.reduce((sum, group) => sum + group.rows.length, 0);
}

/**
 * Замена групп названных заметок свежими: существующая группа заменяется на
 * месте, исчезнувшая убирается, новая дописывается в конец.
 */
function mergeGroups(
  current: readonly SourceGroup[],
  paths: readonly string[],
  found: readonly SourceGroup[],
): SourceGroup[] {
  const named = new Set(paths);
  const fresh = new Map(found.map((group) => [group.path, group]));
  const merged: SourceGroup[] = [];
  for (const group of current) {
    if (!named.has(group.path)) {
      merged.push(group);
      continue;
    }
    const replacement = fresh.get(group.path);
    if (replacement !== undefined) {
      merged.push(replacement);
      fresh.delete(group.path);
    }
  }
  return [...merged, ...fresh.values()];
}

/** Уступить циклу событий между порциями прохода: ввод в редакторе не ждёт (SC-004). */
function yieldToEventLoop(): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, 0));
}

export function useNoteLinks(input: NoteLinksInput): NoteLinks {
  const { client, target, enabled, revision, takeChanges, bulkChanging, mentionsExpanded, onLinked } =
    input;

  // Актуальные значения для асинхронных продолжений: ответ обязан сверяться с
  // тем, что открыто **сейчас**, а не с тем, что было при запросе (FR-009).
  const targetRef = useRef(target);
  targetRef.current = target;
  const clientRef = useRef(client);
  clientRef.current = client;
  const openTextsRef = useRef(input.openTexts);
  openTextsRef.current = input.openTexts;
  const onLinkedRef = useRef(onLinked);
  onLinkedRef.current = onLinked;

  const [backlinksState, setBacklinksState] = useState<BacklinksState | null>(null);
  const [mentionsState, setMentionsState] = useState<MentionsState | null>(null);
  const [stale, setStale] = useState<{
    target: string;
    sources: ReadonlyMap<string, SourceGroup>;
  } | null>(null);
  const [outcomesState, setOutcomesState] = useState<{
    target: string;
    map: ReadonlyMap<string, LinkOutcome>;
  } | null>(null);
  const [linkingKey, setLinkingKey] = useState<string | null>(null);
  // Номер прохода заново: растёт, когда точечного обновления недостаточно.
  const [scanEpoch, setScanEpoch] = useState(0);

  // Номер запроса обратных ссылок: ответ с чужим номером отбрасывается.
  const backlinksRequest = useRef(0);
  const mentionsPhase = useRef<MentionsView["phase"]>("collapsed");

  const active = enabled && client !== null && target !== null;

  // Название цели спрашивается у ядра (`title_of`), а не режется из пути здесь.
  const [titleState, setTitleState] = useState<{ target: string; title: string } | null>(null);
  useEffect(() => {
    if (!active || client === null || target === null) {
      setTitleState(null);
      return undefined;
    }
    let cancelled = false;
    void client.mentionsAvailability(target).then(
      (availability) => {
        if (!cancelled) setTitleState({ target, title: availability.title });
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [active, client, target]);

  // ── Обратные ссылки ──────────────────────────────────────────────────

  const loadFirstPage = useCallback(async (forTarget: string): Promise<void> => {
    const current = clientRef.current;
    if (current === null) return;
    const id = ++backlinksRequest.current;
    try {
      const page = await current.backlinksPage(
        forTarget,
        [...openTextsRef.current],
        0,
        LINKS_PAGE,
      );
      if (id !== backlinksRequest.current || page.target !== targetRef.current) return;
      setBacklinksState({
        target: forTarget,
        view: {
          phase: "ready",
          groups: page.groups,
          totalSources: page.total_sources,
          hasMore: page.next_offset !== undefined,
          loadingMore: false,
        },
      });
    } catch {
      if (id !== backlinksRequest.current) return;
      // Показанный список при неудачном обновлении остаётся: отказ не
      // должен стирать то, что уже видно.
      setBacklinksState((previous) =>
        previous?.target === forTarget && previous.view.phase === "ready"
          ? previous
          : { target: forTarget, view: { phase: "failed" } },
      );
    }
  }, []);

  // Смена цели, показ и скрытие колонки, начало и конец массового изменения:
  // всё сбрасывается до прихода ответа.
  useEffect(() => {
    backlinksRequest.current += 1;
    if (!active || target === null) {
      setBacklinksState(null);
      return;
    }
    setBacklinksState({
      target,
      view: { phase: bulkChanging ? "collecting" : "loading" },
    });
    if (!bulkChanging) void loadFirstPage(target);
  }, [active, target, bulkChanging, loadFirstPage]);

  const showMore = useCallback(() => {
    const current = clientRef.current;
    const forTarget = targetRef.current;
    if (current === null || forTarget === null) return;
    const state = backlinksState;
    if (state?.target !== forTarget || state.view.phase !== "ready" || !state.view.hasMore) return;
    if (state.view.loadingMore) return;
    const loaded = state.view.groups.length;
    const id = ++backlinksRequest.current;
    setBacklinksState({ target: forTarget, view: { ...state.view, loadingMore: true } });
    void (async () => {
      try {
        const page = await current.backlinksPage(
          forTarget,
          [...openTextsRef.current],
          loaded,
          LINKS_PAGE,
        );
        if (id !== backlinksRequest.current || page.target !== targetRef.current) return;
        setBacklinksState((previous) => {
          if (previous?.target !== forTarget || previous.view.phase !== "ready") return previous;
          const known = new Set(previous.view.groups.map((group) => group.path));
          return {
            target: forTarget,
            view: {
              phase: "ready",
              groups: [
                ...previous.view.groups,
                ...page.groups.filter((group) => !known.has(group.path)),
              ],
              totalSources: page.total_sources,
              hasMore: page.next_offset !== undefined,
              loadingMore: false,
            },
          };
        });
      } catch {
        if (id !== backlinksRequest.current) return;
        setBacklinksState((previous) =>
          previous?.target === forTarget && previous.view.phase === "ready"
            ? { target: forTarget, view: { ...previous.view, loadingMore: false } }
            : previous,
        );
      }
    })();
  }, [backlinksState]);

  // ── Упоминания ───────────────────────────────────────────────────────

  const dirty = useRef<{ all: boolean; paths: Set<string> }>({ all: false, paths: new Set() });

  const publishMentions = useCallback((forTarget: string, view: MentionsView) => {
    mentionsPhase.current = view.phase;
    setMentionsState({ target: forTarget, view });
  }, []);

  /** Упоминания только в названных заметках (research.md §8). */
  const refreshMentionNotes = useCallback(
    async (forTarget: string, paths: readonly string[]): Promise<void> => {
      const current = clientRef.current;
      if (current === null || paths.length === 0) return;
      try {
        const found = await current.mentionsIn(forTarget, [...paths], [...openTextsRef.current]);
        if (targetRef.current !== forTarget) return;
        setMentionsState((previous) =>
          previous?.target === forTarget && previous.view.phase === "ready"
            ? {
                target: forTarget,
                view: { phase: "ready", groups: mergeGroups(previous.view.groups, paths, found) },
              }
            : previous,
        );
      } catch {
        // Точечное обновление не удалось — список остаётся прежним; следующее
        // изменение или новый проход его исправят.
      }
    },
    [],
  );

  /** Разобрать накопленные изменения, если проход не идёт. */
  const flushMentions = useCallback(() => {
    const forTarget = targetRef.current;
    if (forTarget === null || mentionsPhase.current !== "ready") return;
    const { all, paths } = dirty.current;
    // Правка самой цели упоминаний не меняет: её в списке нет (FR-006).
    const named = [...paths].filter((path) => path !== forTarget);
    if (!all && named.length === 0) {
      dirty.current = { all: false, paths: new Set() };
      return;
    }
    dirty.current = { all: false, paths: new Set() };
    if (all || named.length > MENTION_POINT_LIMIT) {
      setScanEpoch((epoch) => epoch + 1);
    } else {
      void refreshMentionNotes(forTarget, named);
    }
  }, [refreshMentionNotes]);

  const scanning = active && mentionsExpanded && target !== null;

  useEffect(() => {
    if (!scanning || client === null || target === null) {
      mentionsPhase.current = "collapsed";
      setMentionsState(null);
      setStale(null);
      dirty.current = { all: false, paths: new Set() };
      return undefined;
    }
    const forTarget = target;
    let cancelled = false;
    let scan: MentionScan | null = null;
    dirty.current = { all: false, paths: new Set() };
    setStale(null);

    void (async () => {
      try {
        const availability = await client.mentionsAvailability(forTarget);
        if (cancelled) return;
        if (availability.kind !== "Available") {
          publishMentions(forTarget, { phase: "unavailable", reason: availability });
          return;
        }
        publishMentions(forTarget, { phase: "scanning", scanned: 0, total: 0, groups: [] });
        const started = await client.mentionsStart(forTarget);
        if (cancelled) return;
        if (started.kind === "Unavailable") {
          publishMentions(forTarget, { phase: "unavailable", reason: started.reason });
          return;
        }
        scan = started.state;
        let groups: SourceGroup[] = [];
        while (scan.phase === "Running" && !cancelled) {
          const step = await client.mentionsStep(scan, [...openTextsRef.current]);
          if (cancelled) break;
          scan = step.state;
          groups = [...groups, ...step.found];
          publishMentions(forTarget, {
            phase: "scanning",
            scanned: scan.scanned,
            total: scan.total,
            groups,
          });
          // Между порциями — цикл событий: набор в редакторе не ждёт прохода.
          await yieldToEventLoop();
        }
        if (cancelled) return;
        publishMentions(forTarget, { phase: "ready", groups });
        // Заметки, изменившиеся за время прохода, уточняются точечно.
        flushMentions();
      } catch {
        if (!cancelled) publishMentions(forTarget, { phase: "failed" });
      }
    })();

    return () => {
      cancelled = true;
      // Сворачивание, смена цели и размонтирование отменяют проход (FR-012).
      // Отмена — чистый переход состояния; результат нужен только ядру.
      if (scan !== null) void client.mentionsCancel(scan).catch(() => undefined);
    };
  }, [scanning, client, target, scanEpoch, publishMentions, flushMentions]);

  // ── Обновление по событиям ───────────────────────────────────────────

  // Таймер живёт в ref, а не в эффекте: смена `active` или `bulkChanging` внутри
  // паузы не должна ронять уже запланированное обновление.
  const seenRevision = useRef(revision);
  const takeChangesRef = useRef(takeChanges);
  takeChangesRef.current = takeChanges;
  const refreshTimer = useRef<number | null>(null);
  const refreshNow = useRef<() => void>(() => undefined);
  refreshNow.current = () => {
    const forTarget = targetRef.current;
    if (forTarget === null || !active) return;
    if (!bulkChanging) void loadFirstPage(forTarget);
    flushMentions();
  };

  useEffect(() => {
    if (revision === seenRevision.current) return;
    seenRevision.current = revision;
    const changes = takeChangesRef.current();
    if (changes.all) dirty.current.all = true;
    for (const path of changes.paths) dirty.current.paths.add(path);

    if (refreshTimer.current !== null) window.clearTimeout(refreshTimer.current);
    refreshTimer.current = window.setTimeout(() => {
      refreshTimer.current = null;
      refreshNow.current();
    }, LINKS_REVISION_DEBOUNCE_MS);
  }, [revision]);

  useEffect(
    () => () => {
      if (refreshTimer.current !== null) window.clearTimeout(refreshTimer.current);
    },
    [],
  );

  // ── «Связать» ────────────────────────────────────────────────────────

  const link = useCallback(
    async (source: SourceGroup, row: SiteRow): Promise<void> => {
      const current = clientRef.current;
      const forTarget = targetRef.current;
      if (current === null || forTarget === null) return;
      const key = mentionKey(source.path, row);
      setLinkingKey(key);
      let outcome: LinkOutcome;
      try {
        // 013 T046–T047: когда появится порт `OpenNoteEditors`, здесь будет
        // `editors.textOf(source.path)`; пока текст редактора не передаётся, и
        // ядро пишет файл само либо отказывает (`UnsavedElsewhere`).
        outcome = await current.linkMention(
          { target: forTarget, source: source.path, place: row.place, text: row.text },
          null,
        );
      } catch {
        outcome = { kind: "Failed" };
      }
      setLinkingKey(null);
      if (targetRef.current !== forTarget) return;

      setOutcomesState((previous) => {
        const map = new Map(previous?.target === forTarget ? previous.map : []);
        if (outcome.kind === "Written") map.delete(key);
        else map.set(key, outcome);
        return { target: forTarget, map };
      });

      if (outcome.kind === "Written") {
        onLinkedRef.current(source.path);
      } else if (outcome.kind === "Stale") {
        setStale((previous) => ({
          target: forTarget,
          sources: new Map([
            ...(previous?.target === forTarget ? previous.sources : []),
            [source.path, source],
          ]),
        }));
        // Список этой заметки обновляется сразу: места в нём уже другие.
        void refreshMentionNotes(forTarget, [source.path]);
      }
    },
    [refreshMentionNotes],
  );

  // ── Итог ─────────────────────────────────────────────────────────────

  const backlinks: BacklinksView =
    backlinksState !== null && backlinksState.target === target && active
      ? backlinksState.view
      : { phase: "idle" };
  const mentions: MentionsView =
    mentionsState !== null && mentionsState.target === target && scanning
      ? mentionsState.view
      : { phase: "collapsed" };

  const backlinkCount = useMemo(() => {
    if (backlinks.phase !== "ready") return null;
    return backlinks.hasMore
      ? { unit: "notes" as const, value: backlinks.totalSources }
      : { unit: "links" as const, value: rowCount(backlinks.groups) };
  }, [backlinks]);
  const mentionCount = mentions.phase === "ready" ? rowCount(mentions.groups) : null;

  const staleSources = useMemo<ReadonlyMap<string, SourceGroup>>(
    () => (stale !== null && stale.target === target && scanning ? stale.sources : new Map()),
    [stale, target, scanning],
  );
  const outcomes = useMemo<ReadonlyMap<string, LinkOutcome>>(
    () => (outcomesState !== null && outcomesState.target === target ? outcomesState.map : new Map()),
    [outcomesState, target],
  );

  return {
    backlinks,
    mentions,
    title: titleState !== null && titleState.target === target ? titleState.title : null,
    staleSources,
    outcomes,
    linkingKey,
    backlinkCount,
    mentionCount,
    showMore,
    link,
  };
}
