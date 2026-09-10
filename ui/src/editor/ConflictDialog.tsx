import type { DiffLine, ResolveError } from "../wasm-core/typsidian_core.js";
import { DiffView } from "./DiffView.js";
import {
  describeResolveError,
  type ConflictEntry,
} from "./useConflictResolution.js";

export interface ConflictDialogProps {
  conflict: ConflictEntry | null;
  diff: DiffLine[] | null;
  isResolving: boolean;
  error: ResolveError | null;
  onShowDiff: () => void;
  onHideDiff: () => void;
  onKeepLocal: () => void;
  onAcceptExternal: () => void;
  onDismissError: () => void;
}

function describeSituation(conflict: ConflictEntry): string {
  switch (conflict.kind) {
    case "diskChanged":
      return `Заметка «${conflict.notePath}» изменена вне приложения, а у вас есть несохранённые правки. Ни одна из версий пока не применена.`;
    case "deleted":
      return `Заметка «${conflict.notePath}» удалена вне приложения, а у вас остались несохранённые правки. Они всё ещё в памяти — их можно сохранить заново.`;
    default: {
      const exhaustive: never = conflict.kind;
      return exhaustive;
    }
  }
}

function acceptExternalLabel(conflict: ConflictEntry): string {
  switch (conflict.kind) {
    case "diskChanged":
      return "Принять внешнюю версию";
    case "deleted":
      // Внешней версии не существует: «принять внешнюю» здесь означает
      // согласиться с удалением (FR-025) — называть это «принять версию» было
      // бы неправдой.
      return "Принять удаление и отбросить правки";
    default: {
      const exhaustive: never = conflict.kind;
      return exhaustive;
    }
  }
}

/**
 * Диалог разрешения конфликта редактирования (T076, FR-021…FR-025).
 *
 * Три варианта из FR-022 присутствуют всегда и равноправны; ни один не
 * выбирается по умолчанию и ни один не применяется автоматически. «Посмотреть
 * отличия» не является исходом — после просмотра пользователь возвращается к
 * тому же выбору (FR-024), поэтому кнопки остаются на месте, а diff
 * разворачивается рядом.
 *
 * Диалог намеренно не закрывается «крестиком»: закрытие без выбора — это
 * отказ от разрешения, и его последствия (FR-037) обрабатывает
 * `useWindowCloseGuard`, а не молчаливое исчезновение окна.
 */
export function ConflictDialog({
  conflict,
  diff,
  isResolving,
  error,
  onShowDiff,
  onHideDiff,
  onKeepLocal,
  onAcceptExternal,
  onDismissError,
}: ConflictDialogProps): React.JSX.Element | null {
  if (!conflict) return null;

  return (
    <div
      className="ts-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-label={`Конфликт редактирования: ${conflict.notePath}`}
    >
      <p>{describeSituation(conflict)}</p>

      {error && (
        <div role="alert">
          <p>{describeResolveError(error)}</p>
          <button type="button" onClick={onDismissError}>
            Понятно
          </button>
        </div>
      )}

      <div role="group" aria-label="Что сделать с правками">
        <button type="button" disabled={isResolving} onClick={onKeepLocal}>
          Оставить свою версию
        </button>
        <button type="button" disabled={isResolving} onClick={onAcceptExternal}>
          {acceptExternalLabel(conflict)}
        </button>
        {conflict.kind === "diskChanged" &&
          (diff === null ? (
            <button type="button" disabled={isResolving} onClick={onShowDiff}>
              Посмотреть отличия
            </button>
          ) : (
            <button type="button" onClick={onHideDiff}>
              Скрыть отличия
            </button>
          ))}
      </div>

      {diff !== null && <DiffView diff={diff} />}
    </div>
  );
}
