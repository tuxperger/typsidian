import type { LastVault, OpenVaultError } from "./useVault.js";

export interface OpenVaultProps {
  isLoading: boolean;
  error: OpenVaultError | null;
  onOpen: () => void;
  /**
   * Последний открытый vault (FR-007); `null` — предлагать нечего: его нет,
   * связь с папкой утрачена либо оболочка хранилища устройства не даёт.
   */
  last?: LastVault | null;
  /** Открыть последний vault. Обязателен, если `last` не `null`. */
  onOpenLast?: () => void;
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
 * Поток «открыть vault» (FR-001, FR-007, FR-008). Отмена системного диалога
 * выбора папки — не ошибка (Assumptions), поэтому `PickerCancelled` не
 * показывается пользователю как сбой.
 *
 * Последний vault именно **предлагается**, а не открывается сам — так сказано
 * в FR-007, и так же он ведёт себя на всех трёх клиентах. Молчаливое открытие
 * было бы к тому же невыполнимо в браузере: разрешение на папку там
 * переспрашивается только в обработчике жеста пользователя, и на загрузке
 * страницы приложение показало бы отказ вместо заметок.
 */
export function OpenVault({
  isLoading,
  error,
  onOpen,
  last = null,
  onOpenLast,
}: OpenVaultProps): React.JSX.Element {
  const message = error ? describeOpenVaultError(error) : null;
  return (
    <div>
      {last && (
        <button type="button" onClick={onOpenLast} disabled={isLoading}>
          {isLoading ? "Открываем…" : `Продолжить: ${last.label}`}
        </button>
      )}
      <button type="button" onClick={onOpen} disabled={isLoading}>
        {isLoading ? "Открываем…" : last ? "Выбрать другую папку" : "Открыть vault"}
      </button>
      {message && <p role="alert">{message}</p>}
    </div>
  );
}
