import type {
  CreateError,
  DeleteError,
  RenameError,
} from "../wasm-core/typsidian_core.js";

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
const INVALID_NAME_MESSAGE =
  "Такое имя нельзя использовать: оно не должно быть пустым и не может содержать «/», «\\» или нулевой символ.";
const NAME_CONFLICT_MESSAGE =
  "Заметка с таким именем уже существует в этой папке.";
// FR-036: права на запись отозваны либо vault стал недоступен. Причина
// называется прямо — «не удалось» скрыло бы то, что диск исправен, а операция
// запрещена состоянием доступа, и пользователю нечего «проверять».
const ACCESS_DENIED_MESSAGE =
  "Сейчас в этот vault нельзя писать: права на запись отозваны или папка стала недоступна. Чтение продолжает работать.";

function describeCreateError(error: CreateError): string {
  switch (error.kind) {
    case "NameConflict":
      return NAME_CONFLICT_MESSAGE;
    case "InvalidName":
      return INVALID_NAME_MESSAGE;
    case "AccessDenied":
      return ACCESS_DENIED_MESSAGE;
    case "Io":
      return "Не удалось создать файл на диске — проверьте свободное место и права доступа к vault.";
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

function describeRenameError(error: RenameError): string {
  switch (error.kind) {
    case "NameConflict":
      return NAME_CONFLICT_MESSAGE;
    case "InvalidName":
      return INVALID_NAME_MESSAGE;
    case "NotFound":
      return "Не удалось переименовать: заметка не найдена — возможно, её уже переименовали или удалили в другом месте.";
    case "AccessDenied":
      return ACCESS_DENIED_MESSAGE;
    case "Io":
      return "Не удалось переименовать файл на диске — проверьте права доступа к vault.";
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

function describeDeleteError(error: DeleteError): string {
  switch (error.kind) {
    case "NotFound":
      return "Не удалось удалить: заметка уже не найдена — возможно, её удалили в другом месте.";
    case "AccessDenied":
      return ACCESS_DENIED_MESSAGE;
    case "Io":
      return "Не удалось удалить файл на диске — проверьте права доступа к vault.";
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

function describeVaultOperationError(error: VaultOperationError): string {
  switch (error.operation) {
    case "create":
      return describeCreateError(error.error);
    case "rename":
      return describeRenameError(error.error);
    case "delete":
      return describeDeleteError(error.error);
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
  if (!error) return null;
  return (
    <div role="alert">
      <p>{describeVaultOperationError(error)}</p>
      {onDismiss && (
        <button type="button" onClick={onDismiss}>
          Закрыть
        </button>
      )}
    </div>
  );
}
