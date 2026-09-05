"use client";

import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import "@excalidraw/excalidraw/index.css";
import type { BoardSaveState } from "../wasm-core/typsidian_core.js";

// Принцип VIII конституции: движок — чёрный ящик. Этот файл монтирует его и
// оформляет **контейнер** — рамку, заголовок панели, индикатор сохранения.
// Внутренний DOM движка не стилизуется и не патчится ни отсюда, ни селекторами
// в CSS (contracts/board-engine-bridge.md § Запреты).

/**
 * Движок требует `window` и на сервере не рендерится, поэтому загружается
 * динамически и только после монтирования (research.md §1). Два уровня защиты
 * намеренно: `lazy` не выполняет модуль во время SSR, а флаг `isMounted`
 * гарантирует это и в том случае, если React решит отрисовать `Suspense`
 * иначе.
 */
const ExcalidrawEngine = lazy(async () => {
  const engine = await import("@excalidraw/excalidraw");
  return { default: engine.Excalidraw };
});

export interface BoardCanvasProps {
  /** Имя доски для заголовка панели; `null` — доска не выбрана. */
  name?: string | null;
  /** Содержимое файла доски; `null` — ещё не загружено. */
  content?: string | null;
  /**
   * Номер загруженного содержимого. Движок принимает сцену только при
   * монтировании, поэтому замена версии после конфликта (FR-015) выполняется
   * его пересозданием: правка внутреннего стора движка запрещена принципом VIII.
   */
  contentRevision?: number;
  /** Состояние сохранения — показывается пользователю (FR-009). */
  saveState?: BoardSaveState;
  /** Причина, по которой доску не удалось прочитать (FR-017). */
  readError?: string | null;
  /** Просмотр без инструментов рисования — мобильный клиент (FR-034). */
  viewModeEnabled?: boolean;
  /** Движок сообщил об изменении рисунка. */
  onChange?: (content: string) => void;
}

export function BoardCanvas({
  name = null,
  content = null,
  contentRevision = 0,
  saveState = { state: "Clean" },
  readError = null,
  viewModeEnabled = false,
  onChange,
}: BoardCanvasProps): React.JSX.Element {
  const [isMounted, setIsMounted] = useState(false);

  useEffect(() => {
    setIsMounted(true);
  }, []);

  // Разбор файла — забота движка, не наша: ядро содержимое не интерпретирует
  // (research.md §9). Здесь только распаковка в ту форму, которую движок ждёт
  // на входе, и отказ разбора превращается в отсутствие начальной сцены, а не
  // в исключение.
  const initialData = useMemo(() => parseScene(content), [content]);

  return (
    <section className="ts-pane ts-pane--board" aria-label="Доска">
      <div className="ts-pane__toolbar">
        {name ? (
          <h2 className="ts-pane__title">{name}</h2>
        ) : (
          <span className="ts-eyebrow">Доска</span>
        )}
        <span className="ts-spacer" />
        {viewModeEnabled && (
          <span className="ts-eyebrow" role="note">
            только просмотр
          </span>
        )}
        <SaveIndicator state={saveState} />
      </div>

      <div className="ts-board">
        {readError ? (
          <div className="ts-placeholder">
            <p role="alert">{readError}</p>
          </div>
        ) : isMounted && content !== null ? (
          <Suspense fallback={<BoardLoading />}>
            <ExcalidrawEngine
              key={contentRevision}
              initialData={initialData}
              viewModeEnabled={viewModeEnabled}
              onChange={(elements, appState, files) => {
                if (!onChange || viewModeEnabled) return;
                onChange(serializeScene(elements, appState, files));
              }}
            />
          </Suspense>
        ) : (
          <BoardLoading />
        )}
      </div>
    </section>
  );
}

/** FR-009: состояние сохранения видно пользователю всегда, а не только при отказе. */
function SaveIndicator({ state }: { state: BoardSaveState }): React.JSX.Element {
  switch (state.state) {
    case "Clean":
      return <span className="ts-eyebrow">сохранено</span>;
    case "Dirty":
      return <span className="ts-eyebrow">есть изменения</span>;
    case "Saving":
      return <span className="ts-eyebrow">сохраняем…</span>;
    case "Failed":
      return (
        <span className="ts-eyebrow ts-eyebrow--alert" role="alert">
          не сохранено: {state.reason}
        </span>
      );
    default: {
      const exhaustive: never = state;
      return exhaustive;
    }
  }
}

function BoardLoading(): React.JSX.Element {
  return (
    <div className="ts-placeholder">
      <p role="status">Готовим полотно…</p>
    </div>
  );
}

type EngineProps = React.ComponentProps<typeof ExcalidrawEngine>;
/**
 * `undefined` исключён намеренно: при `exactOptionalPropertyTypes` его нельзя
 * передать в необязательный проп явно, а `null` движок принимает и трактует
 * как «начальной сцены нет».
 */
type InitialData = Exclude<EngineProps["initialData"], undefined>;
type ChangeArguments = Parameters<NonNullable<EngineProps["onChange"]>>;

/**
 * Содержимое файла → начальная сцена движка. Нечитаемый JSON даёт `null`:
 * пустое полотно вместо падения, а сообщение о причине приходит из ядра через
 * `readError` (FR-017).
 */
function parseScene(content: string | null): InitialData {
  if (content === null) return null;
  try {
    const parsed: unknown = JSON.parse(content);
    if (typeof parsed !== "object" || parsed === null) return null;
    return parsed as InitialData;
  } catch {
    return null;
  }
}

/**
 * Сцена движка → содержимое файла в его же формате.
 *
 * Приложение не добавляет в файл доски своих полей (research.md §2): пишется
 * то, что написал бы сам движок, поэтому такой файл открывается на
 * excalidraw.com (принцип VI).
 */
function serializeScene(
  elements: ChangeArguments[0],
  appState: ChangeArguments[1],
  files: ChangeArguments[2],
): string {
  return JSON.stringify({
    type: "excalidraw",
    version: 2,
    source: "typsidian",
    elements,
    appState: { viewBackgroundColor: appState.viewBackgroundColor },
    files,
  });
}
