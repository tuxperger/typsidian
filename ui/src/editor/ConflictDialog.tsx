import type { DiffLine, ResolveError } from "../wasm-core/typsidian_core.js";
import { DiffView } from "./DiffView.js";
import {
  describeResolveError,
  type ConflictEntry,
} from "./useConflictResolution.js";
import { useI18n, type Translate } from "../i18n/index.js";

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

function describeSituation(t: Translate, conflict: ConflictEntry): string {
  switch (conflict.kind) {
    case "diskChanged":
      return t("editor.conflict.diskChanged", { path: conflict.notePath });
    case "deleted":
      return t("editor.conflict.deleted", { path: conflict.notePath });
    default: {
      const exhaustive: never = conflict.kind;
      return exhaustive;
    }
  }
}

function acceptExternalLabel(t: Translate, conflict: ConflictEntry): string {
  switch (conflict.kind) {
    case "diskChanged":
      return t("editor.conflict.acceptExternal");
    case "deleted":
      // Внешней версии не существует: «принять внешнюю» здесь означает
      // согласиться с удалением (FR-025) — называть это «принять версию» было
      // бы неправдой.
      return t("editor.conflict.acceptDeletion");
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
  const { t } = useI18n();
  if (!conflict) return null;

  return (
    <div
      className="ts-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-label={t("editor.conflict.label", { path: conflict.notePath })}
    >
      <p>{describeSituation(t, conflict)}</p>

      {error && (
        <div role="alert">
          <p>{describeResolveError(t, error)}</p>
          <button type="button" onClick={onDismissError}>
            {t("common.ok")}
          </button>
        </div>
      )}

      <div role="group" aria-label={t("editor.conflict.choice")}>
        <button type="button" disabled={isResolving} onClick={onKeepLocal}>
          {t("editor.conflict.keepLocal")}
        </button>
        <button type="button" disabled={isResolving} onClick={onAcceptExternal}>
          {acceptExternalLabel(t, conflict)}
        </button>
        {conflict.kind === "diskChanged" &&
          (diff === null ? (
            <button type="button" disabled={isResolving} onClick={onShowDiff}>
              {t("editor.conflict.showDiff")}
            </button>
          ) : (
            <button type="button" onClick={onHideDiff}>
              {t("editor.conflict.hideDiff")}
            </button>
          ))}
      </div>

      {diff !== null && <DiffView diff={diff} />}
    </div>
  );
}
