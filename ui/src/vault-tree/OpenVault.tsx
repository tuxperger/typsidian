import { useI18n, type Translate } from "../i18n/index.js";
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
function describeOpenVaultError(t: Translate, error: OpenVaultError): string | null {
  switch (error.kind) {
    case "PickerCancelled":
      return null;
    case "PermissionDenied":
      return t("errors.open.permissionDenied");
    case "NotSupported":
      return t("errors.open.notSupported");
    case "TooManyNotes":
      // FR-038: причина называется прямо, и сказано, что приложение цело —
      // отказ касается этой папки, а не работы вообще.
      return t("errors.open.tooManyNotes");
    case "FolderNotFound":
      return t("errors.open.folderNotFound");
    case "NotADirectory":
      return t("errors.open.notADirectory");
    case "OpenFailed":
      return t("errors.open.failed", { detail: error.detail });
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
  const { t } = useI18n();
  const message = error ? describeOpenVaultError(t, error) : null;
  return (
    <div>
      {last && (
        <button type="button" onClick={onOpenLast} disabled={isLoading}>
          {isLoading ? t("tree.open.opening") : t("tree.open.continue", { label: last.label })}
        </button>
      )}
      <button type="button" onClick={onOpen} disabled={isLoading}>
        {isLoading
          ? t("tree.open.opening")
          : last
            ? t("tree.open.chooseAnother")
            : t("tree.open.open")}
      </button>
      {message && <p role="alert">{message}</p>}
    </div>
  );
}
