"use client";

import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from "react";
import "@excalidraw/excalidraw/index.css";
import { useI18n, type Phrase } from "../i18n/index.js";
import type { BoardSaveView } from "./useBoardDocument.js";

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
  saveState?: BoardSaveView;
  /** Причина, по которой доску не удалось прочитать (FR-017). */
  readError?: Phrase | null;
  /** Просмотр без инструментов рисования — клиент без права записи (FR-034). */
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
  const { t, engineLang } = useI18n();
  const [isMounted, setIsMounted] = useState(false);

  useEffect(() => {
    setIsMounted(true);
  }, []);

  /**
   * Обработчик обязан быть стабильным по ссылке.
   *
   * Движок обёрнут в `React.memo` со сравнением, которое пропускает только
   * `initialData` и `UIOptions`, а остальные пропы сверяет поверхностно. Стрелка,
   * созданная прямо в JSX, давала новую ссылку на каждый рендер — движок
   * перерисовывался, звал `onChange`, тот менял состояние сохранения, рендер
   * повторялся. Цикл замыкался без единого действия пользователя.
   */
  const handleEngineChange = useCallback<NonNullable<EngineProps["onChange"]>>(
    (elements, appState, files) => {
      if (!onChange || viewModeEnabled) return;
      onChange(serializeScene(elements, appState, files));
    },
    [onChange, viewModeEnabled],
  );

  // Разбор файла — забота движка, не наша: ядро содержимое не интерпретирует
  // (research.md §9). Здесь только распаковка в ту форму, которую движок ждёт
  // на входе, и отказ разбора превращается в отсутствие начальной сцены, а не
  // в исключение.
  const initialData = useMemo(() => parseScene(content), [content]);

  return (
    <section className="ts-pane ts-pane--board" aria-label={t("boards.label")}>
      <div className="ts-pane__toolbar">
        {name ? (
          <h2 className="ts-pane__title">{name}</h2>
        ) : (
          <span className="ts-eyebrow">{t("boards.label")}</span>
        )}
        <span className="ts-spacer" />
        {viewModeEnabled && (
          <span className="ts-eyebrow" role="note">
            {t("boards.viewOnly")}
          </span>
        )}
        <SaveIndicator state={saveState} />
      </div>

      <div className="ts-board">
        {readError ? (
          <div className="ts-placeholder">
            <p role="alert">{readError(t)}</p>
          </div>
        ) : isMounted && content !== null ? (
          <Suspense fallback={<BoardLoading />}>
            <ExcalidrawEngine
              key={contentRevision}
              initialData={initialData}
              viewModeEnabled={viewModeEnabled}
              // Язык интерфейса самого движка (FR-025). Строка, а не объект:
              // движок сверяет пропы поверхностно, и новая ссылка на каждый
              // рендер замкнула бы цикл перерисовки.
              langCode={engineLang}
              onChange={handleEngineChange}
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
function SaveIndicator({ state }: { state: BoardSaveView }): React.JSX.Element {
  const { t } = useI18n();
  switch (state.state) {
    case "Clean":
      return <span className="ts-eyebrow">{t("boards.save.saved")}</span>;
    case "Dirty":
      return <span className="ts-eyebrow">{t("boards.save.dirty")}</span>;
    case "Saving":
      return <span className="ts-eyebrow">{t("boards.save.saving")}</span>;
    case "Failed":
      return (
        <span className="ts-eyebrow ts-eyebrow--alert" role="alert">
          {t("boards.save.notSaved", { reason: state.reason(t) })}
        </span>
      );
    default: {
      const exhaustive: never = state;
      return exhaustive;
    }
  }
}

function BoardLoading(): React.JSX.Element {
  const { t } = useI18n();
  return (
    <div className="ts-placeholder">
      <p role="status">{t("boards.loading")}</p>
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
