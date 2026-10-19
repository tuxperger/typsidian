"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  Outline,
  OutlineKinds,
  OutlineRequest,
  OutlineView,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";

// Оглавление открытой заметки для режима «Структура» (фича 015,
// contracts/frontend-outline.md § useOutline).
//
// Хук выбирает **момент** вызова ядра и больше ничего (research.md §9): что
// считать заголовком, какие строки видны, какой раздел текущий и куда
// переезжает свёрнутость после правки, решает `structure`. Свёрнутые ключи —
// непрозрачные строки, фильтр — просто текст (принцип I).

/** Пауза после последней правки, по истечении которой текст разбирается заново (SC-003). */
export const OUTLINE_DEBOUNCE_MS = 250;
/** Не чаще раза в столько миллисекунд спрашивать текущий раздел (SC-004). */
export const OUTLINE_CARET_THROTTLE_MS = 100;

type OutlineClient = Pick<VaultClient, "outline" | "outlineView" | "outlineActive" | "outlineCarry">;

export interface UseOutlineInput {
  client: OutlineClient | null;
  /** Открытая заметка Typst; `null` — не открыта или открыта доска. */
  notePath: string | null;
  /** Текст редактора — разбирается он, а не файл на диске. */
  text: string;
  kinds: OutlineKinds;
  /** Режим «Структура» показан. Выключен — ни одного вызова ядра. */
  enabled: boolean;
}

export interface OutlineHandle {
  /** Строки для показа; `null` — первый разбор ещё не вернулся. */
  view: OutlineView | null;
  /** Последнее оглавление — для перехода в предпросмотре (`locateHeading`). */
  outline: Outline | null;
  /** Ключ подсвеченной строки. */
  active: string | null;
  filter: string;
  setFilter: (filter: string) => void;
  /** Свернуть или развернуть ветку строки. */
  toggle: (key: string) => void;
  /** Каретку редактора сдвинули — только запоминает и ставит таймер. */
  noticeCaret: (offset: number) => void;
}

/** Что окно помнит о заметке, пока она открыта (research.md §20). */
interface NoteMemory {
  collapsed: string[];
  filter: string;
}

const EMPTY_MEMORY: NoteMemory = { collapsed: [], filter: "" };

interface Parsed {
  path: string;
  text: string;
  value: Outline;
}

export function useOutline({ client, notePath, text, kinds, enabled }: UseOutlineInput): OutlineHandle {
  const [parsed, setParsed] = useState<Parsed | null>(null);
  const [view, setView] = useState<{ path: string; value: OutlineView } | null>(null);
  const [active, setActive] = useState<string | null>(null);
  // Свёрнутые ветки и фильтр — по пути заметки: вернулся к заметке — всё как
  // было. Между запусками не сохраняются (FR-008).
  const [memory, setMemory] = useState<Record<string, NoteMemory>>({});
  const current = notePath === null ? EMPTY_MEMORY : (memory[notePath] ?? EMPTY_MEMORY);

  // Свежие значения для асинхронных продолжений и таймера каретки: колбэки
  // ядра возвращаются позже рендера, в котором их запустили.
  const parsedRef = useRef<Parsed | null>(null);
  parsedRef.current = parsed;
  const memoryRef = useRef(memory);
  memoryRef.current = memory;
  const requestRef = useRef<OutlineRequest>({});
  const caretRef = useRef<number | null>(null);
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled && client !== null && notePath !== null;
  const wasEnabled = useRef(false);

  const patchMemory = useCallback((path: string, patch: Partial<NoteMemory>) => {
    setMemory((all) => ({ ...all, [path]: { ...(all[path] ?? EMPTY_MEMORY), ...patch } }));
  }, []);

  // Другая заметка — каретка прежней к ней не относится.
  useEffect(() => {
    caretRef.current = null;
    setActive(null);
  }, [notePath]);

  // ── Разбор ────────────────────────────────────────────────────────────
  useEffect(() => {
    const on = enabled && client !== null && notePath !== null;
    const turnedOn = on && !wasEnabled.current;
    wasEnabled.current = on;
    if (!on) return undefined;

    const previous = parsedRef.current;
    // Сразу — при открытии заметки, включении режима и первом настоящем
    // тексте (документ дочитывается после смены пути); иначе — по остановке
    // набора.
    const immediate =
      turnedOn || previous === null || previous.path !== notePath || previous.text === "";
    let stale = false;
    const run = (): void => {
      void (async () => {
        try {
          const next = await client.outline(text);
          if (stale) return;
          const before = parsedRef.current;
          const collapsed = memoryRef.current[notePath]?.collapsed ?? [];
          if (before !== null && before.path === notePath && collapsed.length > 0) {
            const carried = await client.outlineCarry(before.value, next, collapsed);
            if (stale) return;
            patchMemory(notePath, { collapsed: carried });
          }
          setParsed({ path: notePath, text, value: next });
        } catch {
          // Ядро не ответило: остаётся прежнее оглавление, следующая правка
          // спросит заново.
        }
      })();
    };
    if (immediate) {
      run();
      return () => {
        stale = true;
      };
    }
    const timer = window.setTimeout(run, OUTLINE_DEBOUNCE_MS);
    return () => {
      stale = true;
      window.clearTimeout(timer);
    };
  }, [client, notePath, text, enabled, patchMemory]);

  // ── Строки ────────────────────────────────────────────────────────────
  const figures = kinds.figures ?? false;
  const labels = kinds.labels ?? false;
  const collapsedKey = current.collapsed.join("\u0000");
  useEffect(() => {
    if (!enabledRef.current || client === null || parsed === null || parsed.path !== notePath) {
      return undefined;
    }
    const request: OutlineRequest = {
      kinds: { figures, labels },
      collapsed: current.collapsed,
      filter: current.filter,
      ...(caretRef.current === null ? {} : { caret: caretRef.current }),
    };
    requestRef.current = request;
    let stale = false;
    void client
      .outlineView(parsed.value, request)
      .then((value) => {
        if (stale) return;
        setView({ path: parsed.path, value });
        setActive(value.active ?? null);
      })
      .catch(() => undefined);
    return () => {
      stale = true;
    };
    // `current.collapsed` сверяется по содержимому (`collapsedKey`): новый
    // массив с теми же ключами — не повод спрашивать ядро.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client, parsed, notePath, figures, labels, collapsedKey, current.filter, enabled]);

  // ── Текущий раздел ────────────────────────────────────────────────────
  const caretTimer = useRef<number | null>(null);
  useEffect(
    () => () => {
      if (caretTimer.current !== null) window.clearTimeout(caretTimer.current);
    },
    [],
  );
  const noticeCaret = useCallback(
    (offset: number) => {
      caretRef.current = offset;
      // Задний фронт: первый сдвиг ставит таймер, остальные только обновляют
      // значение — спрашивается последнее. Из обработчика нажатия ядро не
      // зовётся никогда.
      if (caretTimer.current !== null || client === null) return;
      caretTimer.current = window.setTimeout(() => {
        caretTimer.current = null;
        const target = parsedRef.current;
        const caret = caretRef.current;
        if (!enabledRef.current || target === null || caret === null) return;
        const request: OutlineRequest = { ...requestRef.current, caret };
        void client
          .outlineActive(target.value, request)
          .then((key) => {
            // Ответ на прежний разбор или прежнюю заметку не нужен.
            if (parsedRef.current === target) setActive(key);
          })
          .catch(() => undefined);
      }, OUTLINE_CARET_THROTTLE_MS);
    },
    [client],
  );

  const setFilter = useCallback(
    (filter: string) => {
      if (notePath !== null) patchMemory(notePath, { filter });
    },
    [notePath, patchMemory],
  );

  const toggle = useCallback(
    (key: string) => {
      if (notePath === null) return;
      const collapsed = memoryRef.current[notePath]?.collapsed ?? [];
      patchMemory(notePath, {
        collapsed: collapsed.includes(key)
          ? collapsed.filter((other) => other !== key)
          : [...collapsed, key],
      });
    },
    [notePath, patchMemory],
  );

  const shown = view !== null && view.path === notePath ? view.value : null;
  const outline = parsed !== null && parsed.path === notePath ? parsed.value : null;
  return useMemo(
    () => ({
      view: shown,
      outline,
      active: shown === null ? null : active,
      filter: current.filter,
      setFilter,
      toggle,
      noticeCaret,
    }),
    [shown, outline, active, current.filter, setFilter, toggle, noticeCaret],
  );
}
