"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  Diagnostic,
  PageSlideInfo,
  ShowAction,
  ShowKeymap,
  ShowState,
  SlideLabel,
} from "../wasm-core/typsidian_core.js";
import type { SlidePage } from "./SlideImage.js";
import type { ShowPageSize, ShowRole, ShowViewer } from "./show-channel.js";

// Хук окна-зрителя (фича 019, contracts/show-channel.md): принимает `hello`, `deck`,
// `state`, `end` от окна редактора, держит кэш SVG по отпечатку и превращает
// клавиши в `action`/`exit`. Состояния показа окно не меняет само — оно приходит
// только в `state` (гарантия канала); ядра в окне нет.

/** Набор номера слайда сбрасывается без `Enter` (A3, research.md §6). */
export const GOTO_RESET_MS = 3000;

interface Deck {
  digests: string[];
  sizes: ShowPageSize[];
  slides: PageSlideInfo[];
}

interface Position {
  state: ShowState;
  label: SlideLabel;
  stale: boolean;
  failure: Diagnostic | undefined;
}

export interface ShowWindowModel {
  /** Получены `hello`, `deck` и `state`: до этого рисуется только фон показа. */
  ready: boolean;
  /** Показ завершён хозяином: окно закрывается. */
  ended: boolean;
  /** Язык интерфейса окна редактора. */
  locale: string | undefined;
  position: Position | null;
  /** Страницы по порядку; SVG подставлен из кэша. */
  pages: SlidePage[];
  /** Сведения о страницах: заметки докладчика и номер слайда. */
  slides: PageSlideInfo[];
  /** Набранные цифры номера слайда; `null` — набора нет. */
  gotoBuffer: string | null;
  act: (action: ShowAction) => void;
  exit: () => void;
  togglePresenter: () => void;
  revealError: () => void;
  /** Окно закрывается самим зрителем (кнопка «Закрыть показ»). */
  close: () => void;
}

/**
 * @param viewer канал окна
 * @param role роль окна; от неё зависит, чьи `deck` принимаются
 */
export function useShowWindow(viewer: ShowViewer, role: ShowRole): ShowWindowModel {
  const [keymap, setKeymap] = useState<ShowKeymap | null>(null);
  const [locale, setLocale] = useState<string | undefined>(undefined);
  const [deck, setDeck] = useState<Deck | null>(null);
  const [position, setPosition] = useState<Position | null>(null);
  const [ended, setEnded] = useState(false);
  const [gotoBuffer, setGotoBuffer] = useState<string | null>(null);
  // Кэш SVG по отпечатку. В ref: приход картинки сам перерисовку не вызывает —
  // её вызывает `deck`, который приходит вместе с ней.
  const cache = useRef(new Map<string, string>());
  const endedRef = useRef(false);

  useEffect(() => {
    const off = viewer.onMessage((message) => {
      switch (message.kind) {
        case "hello":
          setKeymap(message.keymap);
          setLocale(message.locale);
          break;
        case "deck": {
          // `deck` другой роли: у неё свои новые SVG, нам они не нужны.
          if (message.to !== undefined && message.to !== role) break;
          for (const [digest, svg] of Object.entries(message.svgs)) cache.current.set(digest, svg);
          // Выбрасываем то, чего нет в последнем списке (research.md §10).
          const alive = new Set(message.digests);
          for (const digest of [...cache.current.keys()]) {
            if (!alive.has(digest)) cache.current.delete(digest);
          }
          setDeck({ digests: message.digests, sizes: message.sizes, slides: message.slides });
          break;
        }
        case "state":
          setPosition({
            state: message.state,
            label: message.label,
            stale: message.stale,
            failure: message.failure,
          });
          break;
        case "end":
          endedRef.current = true;
          setEnded(true);
          break;
      }
    });
    // Подписка уже стоит — можно просить показ. Отпечатки из кэша избавляют
    // хозяина от повторной отправки (при повторном подключении окна).
    viewer.send({ kind: "ready", role, have: [...cache.current.keys()] });
    return off;
  }, [viewer, role]);

  // Закрытие окна ОС — выход из показа (spec Edge Cases). Хозяин сам закрыл
  // окно (`end`) — докладывать нечего.
  useEffect(() => {
    const onPageHide = (): void => {
      if (!endedRef.current) viewer.send({ kind: "closed", role });
    };
    window.addEventListener("pagehide", onPageHide);
    return () => window.removeEventListener("pagehide", onPageHide);
  }, [viewer, role]);

  const act = useCallback((action: ShowAction) => viewer.send({ kind: "action", action }), [viewer]);
  const exit = useCallback(() => viewer.send({ kind: "exit" }), [viewer]);
  const togglePresenter = useCallback(() => viewer.send({ kind: "togglePresenter" }), [viewer]);
  const revealError = useCallback(() => viewer.send({ kind: "revealError" }), [viewer]);
  const close = useCallback(() => viewer.send({ kind: "closed", role }), [viewer, role]);

  // Набор номера: цифры копятся, `Enter` ведёт к слайду, `Escape` отменяет набор.
  const gotoRef = useRef<string | null>(null);
  const gotoTimer = useRef<number | null>(null);
  const setGoto = useCallback((next: string | null) => {
    gotoRef.current = next;
    setGotoBuffer(next);
    if (gotoTimer.current !== null) window.clearTimeout(gotoTimer.current);
    gotoTimer.current =
      next === null ? null : window.setTimeout(() => setGoto(null), GOTO_RESET_MS);
  }, []);
  useEffect(
    () => () => {
      if (gotoTimer.current !== null) window.clearTimeout(gotoTimer.current);
    },
    [],
  );

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      // На сфокусированной кнопке `Enter` и пробел — её собственное нажатие.
      const onButton = event.target instanceof HTMLButtonElement;
      if (onButton && (event.key === "Enter" || event.key === " ")) return;

      const typing = gotoRef.current;
      if (/^[0-9]$/.test(event.key)) {
        event.preventDefault();
        // Длиннее пяти цифр слайдов не бывает; лишнее не копится.
        setGoto(((typing ?? "") + event.key).slice(0, 5));
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        if (typing !== null) setGoto(null);
        else exit();
        return;
      }
      if (typing !== null && event.key === "Enter") {
        event.preventDefault();
        act({ kind: "goToSlide", number: Number(typing) });
        setGoto(null);
        return;
      }
      if (typing !== null && event.key === "Backspace") {
        event.preventDefault();
        setGoto(typing.length > 1 ? typing.slice(0, -1) : null);
        return;
      }
      const binding = keymap?.bindings.find((entry) => entry.key === event.key);
      if (binding === undefined) return;
      event.preventDefault();
      act(binding.action);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [keymap, act, exit, setGoto]);

  const pages = useMemo<SlidePage[]>(() => {
    if (deck === null) return [];
    return deck.digests.map((digest, index) => {
      const size = deck.sizes[index];
      return {
        digest,
        svg: cache.current.get(digest),
        widthPt: size?.w ?? 0,
        heightPt: size?.h ?? 0,
      };
    });
  }, [deck]);

  return {
    ready: keymap !== null && deck !== null && position !== null,
    ended,
    locale,
    position,
    pages,
    slides: deck?.slides ?? [],
    gotoBuffer,
    act,
    exit,
    togglePresenter,
    revealError,
    close,
  };
}
