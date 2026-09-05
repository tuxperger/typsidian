import type { OpenVaultError } from "./useVault.js";

export interface OpenVaultProps {
  isLoading: boolean;
  error: OpenVaultError | null;
  onOpen: () => void;
}

/**
 * Текст на каждый вариант отказа (FR-008) — без общего «что-то пошло не так».
 * `null` для отмены диалога: пользователь передумал, это не сбой (Assumptions).
 */
function describeOpenVaultError(error: OpenVaultError): string | null {
  switch (error.kind) {
    case "PickerCancelled":
      return null;
    case "PermissionDenied":
      return "Доступ к выбранной папке не предоставлен. Откройте vault заново и разрешите доступ, когда браузер спросит.";
    case "NotSupported":
      return "Этот браузер не умеет открывать локальные папки (нет File System Access API). Подойдёт браузер на основе Chromium — или используйте настольное приложение.";
    case "TooManyNotes":
      // FR-038: причина называется прямо, и сказано, что приложение цело —
      // отказ касается этой папки, а не работы вообще.
      return "В этой папке больше 10 000 заметок — это выше поддерживаемого размера vault, и открыть её нельзя. Выберите папку поменьше или разделите её на несколько vault.";
    case "OpenFailed":
      return `Не удалось открыть vault: ${error.message}`;
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

/**
 * Поток «открыть vault» (FR-001, FR-008). Отмена системного диалога выбора
 * папки — не ошибка (Assumptions), поэтому `PickerCancelled` не показывается
 * пользователю как сбой.
 */
export function OpenVault({ isLoading, error, onOpen }: OpenVaultProps): React.JSX.Element {
  const message = error ? describeOpenVaultError(error) : null;
  return (
    <div>
      <button type="button" onClick={onOpen} disabled={isLoading}>
        {isLoading ? "Открываем…" : "Открыть vault"}
      </button>
      {message && <p role="alert">{message}</p>}
    </div>
  );
}
