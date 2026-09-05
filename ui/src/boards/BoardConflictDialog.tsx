"use client";

import { useEffect, useState } from "react";
import { exportBoardContentToSvg, type SvgExport } from "./exportSvg.js";

// Разрешение конфликта доски (T033, T034, FR-015, FR-016, SC-006).
//
// Отличие от `editor/ConflictDialog.tsx` — только в представлении: обнаружение
// конфликта то же самое, из `vault/conflict.rs` фичи 001 (research.md §7).
// Текстовый diff для рисунка спека запрещает прямо, поэтому вместо него две
// превью рядом.
//
// Обе строятся через общую обёртку `exportSvg.ts` (T015): второго вызова
// `exportToSvg` в проекте нет, иначе поведение при отказе экспорта разошлось бы
// между превью конфликта и снимком вставки.

/** Превью одной версии: строится, построена или не построилась с причиной. */
type Preview = { status: "building" } | { status: "settled"; result: SvgExport };

export interface BoardConflictDialogProps {
  /** Доска, по которой возник конфликт; `null` — диалога нет. */
  boardPath: string | null;
  /** Содержимое буфера пользователя; `null` — буфера нет. */
  localContent: string | null;
  /** Содержимое, прочитанное с диска; `null` — прочитать не удалось. */
  externalContent: string | null;
  /** Идёт запись выбранной версии: повторный выбор до её конца не принимается. */
  isResolving?: boolean;
  /** Отказ разрешения словами; `null` — отказа не было. */
  error?: string | null;
  onKeepLocal: () => void;
  onAcceptExternal: () => void;
  onDismissError?: () => void;
}

/**
 * Диалог конфликта доски.
 *
 * Ни одна версия не выбрана по умолчанию и ни одна не применяется сама: до
 * нажатия кнопки в файл доски не пишется ничего (FR-014, инвариант 1
 * data-model.md). Отказ построения превью выбор не блокирует — он сообщается на
 * месте картинки, а кнопки остаются рабочими (FR-016).
 */
export function BoardConflictDialog({
  boardPath,
  localContent,
  externalContent,
  isResolving = false,
  error = null,
  onKeepLocal,
  onAcceptExternal,
  onDismissError,
}: BoardConflictDialogProps): React.JSX.Element | null {
  const local = useBoardPreview(boardPath === null ? null : localContent);
  const external = useBoardPreview(boardPath === null ? null : externalContent);

  if (boardPath === null) return null;

  return (
    <div
      role="alertdialog"
      aria-modal="true"
      aria-label={`Конфликт доски: ${boardPath}`}
      className="ts-board-conflict"
    >
      <p>
        Доска «{boardPath}» изменена вне приложения, а на полотне есть
        несохранённые правки. Ни одна из версий пока не применена — в файл не
        записано ничего.
      </p>

      {error !== null && (
        <div role="alert">
          <p>{error}</p>
          {onDismissError && (
            <button type="button" onClick={onDismissError}>
              Понятно
            </button>
          )}
        </div>
      )}

      <div className="ts-board-versions">
        <BoardVersion
          title="Ваша версия"
          missing="Буфера с вашими правками уже нет."
          preview={local}
        />
        <BoardVersion
          title="Версия на диске"
          missing="Внешнюю версию не удалось прочитать с диска."
          preview={external}
        />
      </div>

      <div role="group" aria-label="Какую версию оставить">
        <button type="button" disabled={isResolving} onClick={onKeepLocal}>
          Оставить свою версию
        </button>
        <button type="button" disabled={isResolving} onClick={onAcceptExternal}>
          Принять версию с диска
        </button>
      </div>
    </div>
  );
}

/**
 * Одна версия: заголовок и картинка под ним.
 *
 * Три исхода названы словами, и ни один не выглядит как пустой рисунок: пустое
 * место на месте превью пользователь читает как «здесь ничего не нарисовано», а
 * это другое утверждение (FR-016).
 */
function BoardVersion({
  title,
  missing,
  preview,
}: {
  title: string;
  missing: string;
  preview: Preview | null;
}): React.JSX.Element {
  return (
    <figure className="ts-board-version">
      <figcaption className="ts-eyebrow">{title}</figcaption>
      {preview === null ? (
        <p className="ts-board-version__note">{missing}</p>
      ) : preview.status === "building" ? (
        <p className="ts-board-version__note" role="status">
          Строим превью…
        </p>
      ) : preview.result.ok ? (
        <img
          className="ts-board-version__image"
          src={svgDataUrl(preview.result.svg)}
          alt={`Превью: ${title}`}
        />
      ) : (
        <p className="ts-board-version__note" role="alert">
          Превью показать не удалось: {preview.result.reason}. Выбрать эту версию
          всё равно можно.
        </p>
      )}
    </figure>
  );
}

/**
 * Строит превью содержимого. `null` на входе — версии нет, и это не отказ
 * экспорта, а её отсутствие.
 */
function useBoardPreview(content: string | null): Preview | null {
  const [preview, setPreview] = useState<Preview | null>(null);

  useEffect(() => {
    if (content === null) {
      setPreview(null);
      return;
    }
    let cancelled = false;
    setPreview({ status: "building" });
    void (async () => {
      const result = await exportBoardContentToSvg(content);
      if (!cancelled) setPreview({ status: "settled", result });
    })();
    return () => {
      cancelled = true;
    };
  }, [content]);

  return preview;
}

/**
 * SVG показывается через `<img>`, а не вставкой разметки в наш DOM.
 *
 * Файл доски мог быть создан любым инструментом, и SVG умеет носить в себе
 * скрипты и обработчики событий. Внутри `<img>` они не исполняются, а вставка
 * той же разметки в документ дала бы им наш origin — то есть доступ к vault
 * пользователя.
 */
function svgDataUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}
