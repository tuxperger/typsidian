import type {
  CreateError,
  DeleteError,
  RenameError,
} from "../wasm-core/typsidian_core.js";
import { useI18n, type Translate } from "../i18n/index.js";

/**
 * Ошибка одной из операций управления заметками (create/rename/delete),
 * различённая по вызвавшей её операции — не по форме самого объекта ошибки.
 *
 * `CreateError` и `RenameError` пересекаются по вариантам (`NameConflict`,
 * `InvalidName`, `Io`) и структурно неразличимы; `operation` — дискриминант,
 * который вызывающий код обязан проставить сам, зная, какой метод
 * `WebBackend` он вызвал (contracts/vault-backend-trait.md). Это даёт один
 * проп вместо трёх параллельных опциональных — вызывающему коду не нужно
 * следить, что ровно один из трёх выставлен.
 */
export type VaultOperationError =
  | { operation: "create"; error: CreateError }
  | { operation: "rename"; error: RenameError }
  | { operation: "delete"; error: DeleteError };

export interface ErrorBannerProps {
  error: VaultOperationError | null;
  onDismiss?: () => void;
}

// FR-013: имя отклоняется до касания диска (backend.rs::is_valid_note_name) —
// текст объясняет пользователю, чего именно избегать, а не просто "неверное имя".
// FR-036: при отозванных правах причина называется прямо — «не удалось» скрыло
// бы то, что диск исправен, а операция запрещена состоянием доступа.

function describeCreateError(t: Translate, error: CreateError): string {
  switch (error.kind) {
    case "NameConflict":
      return t("errors.nameConflict");
    case "InvalidName":
      return t("errors.invalidName");
    case "AccessDenied":
      return t("errors.accessDenied");
    case "Io":
      return t("errors.create.io");
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

function describeRenameError(t: Translate, error: RenameError): string {
  switch (error.kind) {
    case "NameConflict":
      return t("errors.nameConflict");
    case "InvalidName":
      return t("errors.invalidName");
    case "NotFound":
      return t("errors.rename.notFound");
    case "AccessDenied":
      return t("errors.accessDenied");
    case "Io":
      return t("errors.rename.io");
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

function describeDeleteError(t: Translate, error: DeleteError): string {
  switch (error.kind) {
    case "NotFound":
      return t("errors.delete.notFound");
    case "AccessDenied":
      return t("errors.accessDenied");
    case "Io":
      return t("errors.delete.io");
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

function describeVaultOperationError(t: Translate, error: VaultOperationError): string {
  switch (error.operation) {
    case "create":
      return describeCreateError(t, error.error);
    case "rename":
      return describeRenameError(t, error.error);
    case "delete":
      return describeDeleteError(t, error.error);
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

/**
 * FR-013, FR-014, contracts/frontend-events.md: показывает причину отказа
 * create/rename/delete конкретным текстом на каждый вариант ошибки — без
 * общего «что-то пошло не так». Исчерпывающий `switch` над каждым union
 * (проверено `never` в `default`) гарантирует, что TS strict поймает
 * необработанный вариант, если ядро добавит новый.
 */
export function ErrorBanner({
  error,
  onDismiss,
}: ErrorBannerProps): React.JSX.Element | null {
  const { t } = useI18n();
  if (!error) return null;
  return (
    <div role="alert">
      <p>{describeVaultOperationError(t, error)}</p>
      {onDismiss && (
        <button type="button" onClick={onDismiss}>
          {t("common.close")}
        </button>
      )}
    </div>
  );
}
