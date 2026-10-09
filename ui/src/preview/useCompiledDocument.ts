"use client";

import { useCallback, useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import type {
  CompiledDocument,
  Diagnostic,
  ResolvedAnchor,
  VaultOverrides,
} from "../wasm-core/typsidian_core.js";
import type { PreviewClient } from "../preview-client.js";
import { useScrollAnchor } from "./useScrollAnchor.js";

// Состояние сборки открытой заметки (005, T046): запрос сборки по мере набора,
// отбрасывание опоздавших результатов, отмена при смене заметки, слежение за
// связанными файлами и удержание места прокрутки.
//
// Правил здесь нет (принцип I): что считать тем же местом документа, какие
// файлы связаны и изменились ли они, решает ядро. Хук решает только, когда
// спросить, и что из ответов ещё имеет смысл показывать.

/** Пауза после остановки набора перед сборкой (FR-008). */
export const PREVIEW_DEBOUNCE_MS = 250;

/** Как часто спрашивается, не изменились ли связанные файлы (FR-020). */
// Секунда, а не больше: порог SC-006 — 2 с от правки файла до обновлённого
// вида, и в него должны уложиться такт опроса, пауза дебаунса и сама сборка.
const DEPENDENCY_POLL_MS = 1000;

/** Что показывает панель прямо сейчас (data-model.md «PreviewState»). */
export interface PreviewState {
  /** Последний **успешно** собранный документ. */
  shown: CompiledDocument | null;
  /** `shown` не соответствует текущему тексту (FR-014). */
  isStale: boolean;
  isBuilding: boolean;
  diagnostics: Diagnostic[];
  /**
   * Сборка не состоялась вовсе — вызов ядра отказал, диагностик нет. Здесь
   * техническое описание отказа как есть: формулирует его не приложение.
   */
  failure: string | null;
}

const INITIAL: PreviewState = {
  shown: null,
  isStale: false,
  isBuilding: false,
  diagnostics: [],
  failure: null,
};

export interface CompiledDocumentInput {
  client: PreviewClient | null;
  notePath: string | null;
  text: string;
  /** Смена значения пересобирает заметку без правки текста. */
  rebuildToken: number;
  vaultOverrides: VaultOverrides | null;
  /** Область прокрутки предпросмотра — с неё снимается якорь места. */
  scroller: RefObject<HTMLElement | null>;
  onDependencies?: ((paths: string[]) => void) | undefined;
}

export interface CompiledDocumentControl {
  state: PreviewState;
  /** Сколько длился последний вызов ядра, мс. */
  lastMs: number | null;
  /** Пересобрать сейчас: кнопка, меню, установленный шаблон. */
  rebuild: () => void;
}

export function useCompiledDocument({
  client,
  notePath,
  text,
  rebuildToken,
  vaultOverrides,
  scroller: previewRef,
  onDependencies,
}: CompiledDocumentInput): CompiledDocumentControl {
  const [state, setState] = useState<PreviewState>(INITIAL);
  const [lastMs, setLastMs] = useState<number | null>(null);
  // Колбэк в ref: новая ссылка на него не должна перезапускать сборку.
  const onDependenciesRef = useRef(onDependencies);
  onDependenciesRef.current = onDependencies;
  // Зависимости последней сборки — удачной или нет: недостающий файл тоже в
  // их числе, и его появление обязано пересобрать заметку.
  const dependenciesRef = useRef<string[]>([]);
  // Отметка состояния этих файлов на момент сборки; `null` — ещё не снята.
  const dependencyMarkRef = useRef<string | null>(null);
  const [builtRound, setBuiltRound] = useState(0);
  // Заметка, открытая сейчас, — чтобы опоздавший результат чужой сборки не
  // попал на экран.
  const currentNoteRef = useRef(notePath);
  currentNoteRef.current = notePath;
  // Удержание места при пересборке (FR-004). Якорь снимается перед сборкой и
  // уезжает в ядро вместе с текстом; куда прокрутить, говорит её результат.
  const scrollAnchor = useScrollAnchor(previewRef);
  const shownNoteRef = useRef<string | null>(null);
  const pendingScrollRef = useRef<
    { kind: "top" } | { kind: "anchor"; resolved: ResolvedAnchor; offset: number } | null
  >(null);
  // Монотонная версия текста: результат более старой сборки не должен вытеснять
  // более новый (FR-009). Ядро помечает результат этой же версией.
  const revisionRef = useRef(0);
  const shownRevisionRef = useRef(-1);
  // Пересборка без правки текста — кнопка, меню, установленный шаблон.
  const [menuRound, setMenuRound] = useState(0);
  const rebuild = useCallback(() => setMenuRound((round) => round + 1), []);

  useEffect(() => {
    if (!client || notePath === null) {
      setState(INITIAL);
      shownRevisionRef.current = -1;
      shownNoteRef.current = null;
      dependenciesRef.current = [];
      onDependenciesRef.current?.([]);
      return;
    }

    const revision = ++revisionRef.current;
    setState((prev) => ({ ...prev, isBuilding: true }));

    const timer = window.setTimeout(() => {
      void (async () => {
        // Время меряется вокруг вызова ядра: это ровно то, сколько пользователь
        // ждал результат. Своего поля с длительностью `CompiledDocument` не
        // несёт, а выдумывать его в ядре ради надписи незачем.
        const startedAt = performance.now();
        // Якорь имеет смысл только внутри одной заметки: при переходе к другой
        // «то же место» не существует, и она открывается с начала.
        const captured = shownNoteRef.current === notePath ? scrollAnchor.capture() : null;
        try {
          // Путь с доустановкой шаблонов — только когда клиент его умеет и
          // настройки vault прочитаны. Иначе обычная сборка: заметка без
          // подключений в обоих случаях в сеть не заходит (007 FR-008).
          const document =
            client.compileWithTemplates && vaultOverrides
              ? await client.compileWithTemplates(
                  notePath,
                  text,
                  revision,
                  vaultOverrides,
                  captured?.anchor,
                )
              : await client.compile(notePath, text, revision, captured?.anchor);
          // Пока шла сборка, открыли другую заметку: результат чужой, и
          // показывать его нечему (FR-011). Сюда же приходит отменённая сборка.
          if (currentNoteRef.current !== notePath) return;
          setLastMs(Math.round(performance.now() - startedAt));
          // Пришёл результат для текста, который уже устарел, — выбрасываем.
          if (document.revision < shownRevisionRef.current) return;
          shownRevisionRef.current = document.revision;
          dependenciesRef.current = document.dependencies;
          dependencyMarkRef.current = null;
          setBuiltRound((round) => round + 1);
          if (document.outcome === "Succeeded") {
            if (shownNoteRef.current !== notePath) pendingScrollRef.current = { kind: "top" };
            else if (captured !== null && document.anchor !== undefined) {
              pendingScrollRef.current = {
                kind: "anchor",
                resolved: document.anchor,
                offset: captured.offset,
              };
            }
            shownNoteRef.current = notePath;
          }
          onDependenciesRef.current?.(document.dependencies);

          setState((prev) => ({
            // Успешная сборка заменяет вид; неуспешная оставляет прежний и
            // помечает его несоответствующим тексту (FR-014).
            shown: document.outcome === "Succeeded" ? document : prev.shown,
            isStale: document.outcome !== "Succeeded",
            isBuilding: false,
            diagnostics: document.diagnostics,
            failure: null,
          }));
        } catch (cause) {
          if (currentNoteRef.current !== notePath) return;
          setState((prev) => ({
            ...prev,
            isBuilding: false,
            isStale: true,
            diagnostics: [],
            failure: String(cause),
          }));
        }
      })();
    }, PREVIEW_DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [client, notePath, text, rebuildToken, vaultOverrides, menuRound]);

  // Прокрутка — после того как новые страницы встали в разметку и до того, как
  // браузер их нарисует: иначе вид на кадр дёрнулся бы к старому смещению.
  useLayoutEffect(() => {
    const pending = pendingScrollRef.current;
    pendingScrollRef.current = null;
    if (pending === null) return;
    if (pending.kind === "top") {
      if (previewRef.current !== null) previewRef.current.scrollTop = 0;
    } else {
      scrollAnchor.restore(pending.resolved, pending.offset);
    }
  }, [state.shown, scrollAnchor]);

  // Смена заметки и закрытие панели прекращают идущую сборку (FR-011): её
  // результат уже никому не нужен, а компилятор освобождается для новой.
  useEffect(() => () => client?.cancel?.(), [client, notePath]);

  // Слежение за связанными файлами (FR-020). О правке заметок и досок vault
  // сообщает событием — на него откликается рабочая область. Изображений и
  // файлов данных в его индексе нет, поэтому за ними панель следит сама: раз в
  // пару секунд спрашивает у ядра отметку их состояния и пересобирает
  // заметку, когда та изменилась. Что входит в отметку, знает ядро.
  useEffect(() => {
    const mark = client?.dependencyMark?.bind(client);
    if (mark === undefined || notePath === null) return undefined;
    let stopped = false;
    let asking = false;
    const check = async (): Promise<void> => {
      const paths = dependenciesRef.current;
      if (asking || paths.length === 0) return;
      asking = true;
      try {
        const current = await mark(paths);
        if (stopped || paths !== dependenciesRef.current) return;
        if (dependencyMarkRef.current === null) dependencyMarkRef.current = current;
        else if (dependencyMarkRef.current !== current) {
          // Новую отметку снимет проверка после пересборки.
          dependencyMarkRef.current = null;
          setMenuRound((round) => round + 1);
        }
      } catch {
        // Vault закрывается или недоступен — спросим на следующем такте.
      } finally {
        asking = false;
      }
    };
    // Первая проверка — сразу после сборки: она снимает отметку, с которой
    // сравниваются следующие.
    void check();
    const timer = window.setInterval(() => void check(), DEPENDENCY_POLL_MS);
    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [client, notePath, builtRound]);

  return { state, lastMs, rebuild };
}
