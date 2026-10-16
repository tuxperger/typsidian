"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type {
  CompiledDocument,
  Diagnostic,
  ShowKeymap,
  ShowState,
  SlideLabel,
} from "../wasm-core/typsidian_core.js";
import type { PreviewState } from "../preview/useCompiledDocument.js";
import type { SlidesClient } from "../vault-client.js";
import type { HostMessage, ShowHost, ShowRole, ViewerMessage } from "./show-channel.js";

// Сессия показа в окне редактора (фича 019, research.md §7, contracts/frontend-show.md).
//
// Окно редактора владеет `ShowState`: вызывает ядро (`slides.*`), хранит последний
// удачный документ показываемой заметки и рассылает окнам показа `state`/`deck`.
// Правил показа здесь нет (принцип I) — только очерёдность вызовов и рассылка.

/** Сборка заметки, с которой связан показ. */
export interface PreviewSnapshot {
  notePath: string;
  state: PreviewState;
}

/** Место в заметке для перехода; строка и колонка с единицы, как у `reveal`. */
export interface ShowRevealPlace {
  path: string;
  line: number;
  column: number;
}

export interface ShowSessionInput {
  /** Хозяин канала от оболочки; `null` — показ в этой оболочке недоступен. */
  host: ShowHost | null;
  slides: SlidesClient | null;
  /** Открытая заметка `.typ`; `null` — показывать нечего. */
  notePath: string | null;
  /** Последняя сборка открытой заметки (из `useCompiledDocument`). */
  preview: PreviewSnapshot | null;
  /** Смещение каретки редактора (UTF-16); `undefined` — редактора нет. */
  caret: () => number | undefined;
  /** Язык интерфейса — окнам показа. */
  locale: string;
  /** Поставить каретку и показать место в редакторе. */
  reveal: (place: ShowRevealPlace) => void;
  /** Вернуть фокус редактору после показа. */
  focusEditor: () => void;
  /** Окно показа не открылось; `retry` повторяет по жесту пользователя. */
  onBlocked: (retry: () => void) => void;
}

export interface ShowSessionApi {
  active: boolean;
  start(from: "caret" | "first", withPresenter: boolean): Promise<void>;
  /** Завершить показ с переходом к месту слайда (вызывается и на `exit`/`closed`). */
  stop(): void;
}

interface Session {
  notePath: string;
  /** `null` — показывать нечего (A5): колоды нет. */
  state: ShowState | null;
  startedAt: number;
  doc: CompiledDocument | null;
  stale: boolean;
  failure: Diagnostic | undefined;
  ended: boolean;
  keymap: ShowKeymap | null;
  /** Отпечатки, которые уже есть у окна роли (research.md §10). */
  sent: Map<ShowRole, Set<string>>;
  open: Set<ShowRole>;
  /** Обработчики идут по очереди: ответы ядра не обгоняют друг друга. */
  queue: Promise<void>;
  off: () => void;
}

function isEmptyDeck(cause: unknown): boolean {
  return (
    typeof cause === "object" && cause !== null && "kind" in cause && cause.kind === "EmptyDeck"
  );
}

/** Причина «показывать нечего» для A5: ядро отказалось от пустой колоды. */
function emptyDeckDiagnostic(): Diagnostic {
  return {
    severity: "Error",
    message: "no pages to show",
    location: undefined,
    app: { code: "SlidesEmptyDeck" },
  };
}

/** Первая ошибка сборки; отказ самого вызова (диагностик нет) — как замечание без места. */
function firstError(preview: PreviewState): Diagnostic | undefined {
  const error = preview.diagnostics.find((entry) => entry.severity === "Error");
  if (error !== undefined) return error;
  if (preview.failure !== null) {
    return { severity: "Error", message: preview.failure, location: undefined };
  }
  return preview.diagnostics[0];
}

const NO_LABEL: SlideLabel = {
  current: 0,
  total: 0,
  step: undefined,
  steps: undefined,
  next_page: undefined,
};

export function useShowSession(input: ShowSessionInput): ShowSessionApi {
  const inputRef = useRef(input);
  inputRef.current = input;
  const session = useRef<Session | null>(null);
  const [active, setActive] = useState(false);

  /** Сообщение окнам; адресат `deck` решает транспорт. */
  const send = (message: HostMessage): void => inputRef.current.host?.send(message);

  const sendState = async (s: Session): Promise<void> => {
    const { slides } = inputRef.current;
    if (slides === null) return;
    const state = s.state ?? { page: 0, total: 0, started_at_ms: s.startedAt };
    const label =
      s.state === null || s.doc === null ? NO_LABEL : await slides.label(s.doc.slides ?? [], s.state.page);
    send({
      kind: "state",
      state,
      label,
      stale: s.stale,
      ...(s.failure === undefined ? {} : { failure: s.failure }),
    });
  };

  /** Колода окнам: SVG — только те, которых у окна ещё нет. */
  const sendDeck = (s: Session, roles: Iterable<ShowRole>): void => {
    const pages = s.state === null || s.doc === null ? [] : s.doc.pages;
    for (const role of roles) {
      const have = s.sent.get(role) ?? new Set<string>();
      const svgs: Record<string, string> = {};
      for (const page of pages) {
        if (!have.has(page.digest)) svgs[page.digest] = page.svg;
      }
      s.sent.set(role, new Set(pages.map((page) => page.digest)));
      send({
        kind: "deck",
        to: role,
        digests: pages.map((page) => page.digest),
        svgs,
        sizes: pages.map((page) => ({ w: page.width_pt, h: page.height_pt })),
        slides: s.state === null || s.doc === null ? [] : (s.doc.slides ?? []),
      });
    }
  };

  const finish = async (s: Session, withReveal: boolean): Promise<void> => {
    if (s.ended) return;
    s.ended = true;
    s.off();
    const { host, slides } = inputRef.current;
    let place: Awaited<ReturnType<SlidesClient["caret"]>>;
    if (withReveal && s.state !== null && s.doc !== null && slides !== null) {
      place = await slides.caret(s.doc.slides ?? [], s.state.page).catch(() => undefined);
    }
    send({ kind: "end" });
    await Promise.all([...s.open].map((role) => host?.close(role)));
    if (session.current === s) {
      session.current = null;
      setActive(false);
    }
    if (!withReveal) return;
    // Каретка — на место показанного слайда (FR-004). Слайда нет (A5) — фокус
    // возвращается на заметку как есть.
    if (place !== undefined) {
      inputRef.current.reveal({ path: s.notePath, line: place.line + 1, column: place.column + 1 });
    }
    inputRef.current.focusEditor();
  };

  const handle = async (s: Session, message: ViewerMessage): Promise<void> => {
    const { slides, host } = inputRef.current;
    if (s.ended || slides === null || host === null) return;
    switch (message.kind) {
      case "ready": {
        s.open.add(message.role);
        s.sent.set(message.role, new Set(message.have));
        s.keymap ??= await slides.keymap();
        send({ kind: "hello", keymap: s.keymap, locale: inputRef.current.locale });
        sendDeck(s, [message.role]);
        await sendState(s);
        break;
      }
      case "action": {
        if (s.state === null || s.doc === null) break;
        s.state = await slides.navigate(s.state, s.doc.slides ?? [], message.action);
        await sendState(s);
        break;
      }
      case "exit":
        await finish(s, true);
        break;
      case "closed":
        // Окно показа закрыто ОС — то же, что выход; вид докладчика закрывает только себя.
        if (message.role === "show") await finish(s, true);
        else {
          s.open.delete("presenter");
          s.sent.delete("presenter");
        }
        break;
      case "togglePresenter": {
        if (s.open.has("presenter")) {
          s.open.delete("presenter");
          s.sent.delete("presenter");
          await host.close("presenter");
        } else if (await host.open("presenter")) {
          s.open.add("presenter");
        } else {
          inputRef.current.onBlocked(() => void host.open("presenter"));
        }
        break;
      }
      case "revealError": {
        const place = s.failure?.location;
        await finish(s, false);
        if (place !== undefined) {
          inputRef.current.reveal({ path: place.path, line: place.line, column: place.column });
        }
        inputRef.current.focusEditor();
        break;
      }
    }
  };

  const enqueue = (s: Session, task: () => Promise<void>): void => {
    s.queue = s.queue.then(task).catch(() => undefined);
  };

  const start = useCallback(async (from: "caret" | "first", withPresenter: boolean): Promise<void> => {
    const { host, slides, notePath, preview } = inputRef.current;
    if (host === null || slides === null || notePath === null) return;
    const previous = session.current;
    if (previous !== null) await finish(previous, false);

    const snapshot = preview !== null && preview.notePath === notePath ? preview.state : null;
    const s: Session = {
      notePath,
      state: null,
      startedAt: Date.now(),
      doc: snapshot !== null && snapshot.shown?.note_path === notePath ? snapshot.shown : null,
      stale: snapshot?.isStale ?? false,
      failure: undefined,
      ended: false,
      keymap: null,
      sent: new Map(),
      open: new Set(),
      queue: Promise.resolve(),
      off: () => undefined,
    };
    session.current = s;
    s.off = host.onMessage((message) => enqueue(s, () => handle(s, message)));

    // Окна открываются первыми и синхронно с жестом пользователя: после первого
    // `await` блокировщик окон уже не считает вызов откликом на нажатие.
    const opening: Promise<boolean>[] = [host.open("show")];
    const wantsPresenter = withPresenter && host.supports.presenter;
    if (wantsPresenter) opening.push(host.open("presenter"));

    // Колода и первое состояние — до ответов на `ready`: очередь стоит за этим.
    enqueue(s, async () => {
      const caret = from === "caret" ? inputRef.current.caret() : undefined;
      const slideInfo = s.doc?.slides ?? [];
      if (s.doc !== null && slideInfo.length > 0) {
        try {
          s.state = await slides.start(slideInfo, caret, s.startedAt);
        } catch (cause) {
          if (!isEmptyDeck(cause)) throw cause;
        }
      }
      if (s.state === null) {
        s.failure = (snapshot === null ? undefined : firstError(snapshot)) ?? emptyDeckDiagnostic();
      } else if (s.stale && snapshot !== null) {
        s.failure = firstError(snapshot);
      }
    });
    setActive(true);

    const [showOpened, presenterOpened] = await Promise.all(opening);
    if (showOpened !== true) {
      // Окно не открылось: показ не начат, редактор остаётся как был.
      const retryPresenter = presenterOpened === true;
      await finish(s, false);
      if (retryPresenter) await host.close("presenter");
      inputRef.current.onBlocked(() => void start(from, withPresenter));
      return;
    }
    s.open.add("show");
    if (wantsPresenter) {
      if (presenterOpened === true) s.open.add("presenter");
      // Второе окно по тому же жесту браузер мог не пустить: показ идёт, вид
      // докладчика предлагается открыть ещё раз.
      else inputRef.current.onBlocked(() => void host.open("presenter"));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const stop = useCallback((): void => {
    const s = session.current;
    if (s !== null) enqueue(s, () => finish(s, true));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Сборка заметки показа изменилась: перенести номер (FR-007) или пометить A4.
  const preview = input.preview;
  useEffect(() => {
    const s = session.current;
    if (s === null || s.ended || preview === null || preview.notePath !== s.notePath) return;
    enqueue(s, async () => {
      const slides = inputRef.current.slides;
      if (s.ended || slides === null) return;
      const { state: built } = preview;
      // Документ другой заметки (предпросмотр ещё не пересобран после смены) не берём.
      const changedDoc =
        built.shown !== null && built.shown.note_path === s.notePath && built.shown !== s.doc;
      const stale = built.isStale;
      const failure = stale ? firstError(built) : undefined;

      if (changedDoc && built.shown !== null) {
        s.doc = built.shown;
        const info = built.shown.slides ?? [];
        try {
          s.state =
            s.state === null
              ? await slides.start(info, undefined, s.startedAt)
              : await slides.carry(s.state, info);
          s.failure = failure;
        } catch (cause) {
          if (!isEmptyDeck(cause)) throw cause;
          // Правка оставила документ без страниц: A5 в тех же окнах, показ не закрывается.
          s.state = null;
          s.failure = emptyDeckDiagnostic();
        }
        s.stale = stale;
        sendDeck(s, s.open);
        await sendState(s);
        return;
      }

      const next = s.state === null ? (failure ?? firstError(built) ?? s.failure) : failure;
      if (stale !== s.stale || next?.message !== s.failure?.message) {
        s.stale = stale;
        s.failure = next;
        await sendState(s);
      }
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preview]);

  // Окно редактора закрывается или vault меняется — окна показа закрываются без перехода.
  useEffect(
    () => () => {
      const s = session.current;
      if (s !== null) void finish(s, false);
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  return { active, start, stop };
}
