"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import type {
  CompiledDocument,
  Diagnostic,
  RenderedPage,
} from "../wasm-core/typsidian_core.js";
import type { PreviewClient } from "../preview-client.js";

// Принцип I: панель ничего не решает про документ. «Собрался или нет»,
// «где ошибка», «какие страницы» — всё приходит из ядра готовым. Здесь только
// показ и ритм запросов.

/**
 * Пауза после последнего нажатия, по истечении которой запрашивается сборка.
 *
 * Дебаунс живёт в UI, потому что он про ритм ввода, а не про правила сборки
 * (contracts/frontend-preview.md). Значение выбрано под SC-002 «обновление ≤1 с
 * после остановки»: пауза плюс сама сборка обязаны уложиться в бюджет.
 */
export const PREVIEW_DEBOUNCE_MS = 250;

export interface PreviewPaneProps {
  client: PreviewClient | null;
  notePath: string | null;
  /** Текст из буфера редактора, включая несохранённые правки (FR-002). */
  text: string;
}

/** Что показывает панель прямо сейчас (data-model.md «PreviewState»). */
interface PreviewState {
  /** Последний **успешно** собранный документ. */
  shown: CompiledDocument | null;
  /** `shown` не соответствует текущему тексту (FR-014). */
  isStale: boolean;
  isBuilding: boolean;
  diagnostics: Diagnostic[];
}

const INITIAL: PreviewState = {
  shown: null,
  isStale: false,
  isBuilding: false,
  diagnostics: [],
};

export function PreviewPane({
  client,
  notePath,
  text,
}: PreviewPaneProps): React.JSX.Element {
  const [state, setState] = useState<PreviewState>(INITIAL);
  // Монотонная версия текста: результат более старой сборки не должен вытеснять
  // более новый (FR-009). Ядро помечает результат этой же версией.
  const revisionRef = useRef(0);
  const shownRevisionRef = useRef(-1);

  useEffect(() => {
    if (!client || notePath === null) {
      setState(INITIAL);
      shownRevisionRef.current = -1;
      return;
    }

    const revision = ++revisionRef.current;
    setState((prev) => ({ ...prev, isBuilding: true }));

    const timer = window.setTimeout(() => {
      void (async () => {
        try {
          const document = await client.compile(notePath, text, revision);
          // Пришёл результат для текста, который уже устарел, — выбрасываем.
          if (document.revision < shownRevisionRef.current) return;
          shownRevisionRef.current = document.revision;

          setState((prev) => ({
            // Успешная сборка заменяет вид; неуспешная оставляет прежний и
            // помечает его несоответствующим тексту (FR-014).
            shown: document.outcome === "Succeeded" ? document : prev.shown,
            isStale: document.outcome !== "Succeeded",
            isBuilding: false,
            diagnostics: document.diagnostics,
          }));
        } catch (cause) {
          setState((prev) => ({
            ...prev,
            isBuilding: false,
            isStale: true,
            diagnostics: [
              {
                severity: "Error",
                message: `Сборка не состоялась: ${String(cause)}`,
                location: undefined,
              },
            ],
          }));
        }
      })();
    }, PREVIEW_DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [client, notePath, text]);

  const errors = useMemo(
    () => state.diagnostics.filter((d) => d.severity === "Error"),
    [state.diagnostics],
  );
  const warnings = useMemo(
    () => state.diagnostics.filter((d) => d.severity === "Warning"),
    [state.diagnostics],
  );

  return (
    <section className="ts-pane ts-pane--preview" aria-label="Предпросмотр">
      <div className="ts-pane__toolbar">
        <span className="ts-eyebrow">Предпросмотр</span>
        <span className="ts-spacer" />
        {state.isBuilding && <span className="ts-tag">собирается…</span>}
        {state.isStale && !state.isBuilding && (
          <span className="ts-tag">не соответствует тексту</span>
        )}
      </div>

      <div className="ts-preview">
        {state.shown ? (
          state.shown.pages.map((page) => <PreviewPage key={page.index} page={page} />)
        ) : (
          // Пустая панель без объяснения запрещена (FR-014, SC-004).
          <div className="ts-placeholder">
            {notePath === null ? (
              <p>Откройте заметку, чтобы увидеть её свёрстанный вид.</p>
            ) : errors.length > 0 ? (
              <p>Показывать нечего: заметка ещё ни разу не собралась.</p>
            ) : (
              <p>Собирается…</p>
            )}
          </div>
        )}
      </div>

      {(errors.length > 0 || warnings.length > 0) && (
        <ul className="ts-diagnostics" aria-label="Замечания к заметке">
          {errors.map((d, i) => (
            <DiagnosticItem key={`e${i}`} diagnostic={d} />
          ))}
          {warnings.map((d, i) => (
            <DiagnosticItem key={`w${i}`} diagnostic={d} />
          ))}
        </ul>
      )}
    </section>
  );
}

/**
 * Одна страница.
 *
 * SVG вставляется как разметка: это вывод компилятора, а не пользовательский
 * ввод — он порождён ядром из текста заметки и никуда наружу не ходит
 * (принцип IX: содержимое заметок не покидает устройство).
 */
function PreviewPage({ page }: { page: RenderedPage }): React.JSX.Element {
  return (
    <article
      className="ts-preview__page"
      aria-label={`Страница ${page.number}`}
      style={{ aspectRatio: `${page.width_pt} / ${page.height_pt}` }}
      dangerouslySetInnerHTML={{ __html: page.svg }}
    />
  );
}

function DiagnosticItem({
  diagnostic,
}: {
  diagnostic: Diagnostic;
}): React.JSX.Element {
  const place = diagnostic.location;
  return (
    <li
      className={
        diagnostic.severity === "Error"
          ? "ts-diagnostics__error"
          : "ts-diagnostics__warning"
      }
    >
      <span>{diagnostic.message}</span>
      {place && (
        <span className="ts-diagnostics__place">
          {" "}
          — {place.path}:{place.line}:{place.column}
        </span>
      )}
    </li>
  );
}
