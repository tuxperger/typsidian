import type { AccessState } from "../wasm-core/typsidian_core.js";

export interface AccessStateBannerProps {
  accessState: AccessState | null;
  /** FR-018: восстановление из `Unavailable` — только явным переоткрытием. */
  onReopenVault?: () => void;
}

/**
 * Баннеры состояния доступа к vault (T061, FR-020, FR-036).
 *
 * Два состояния показываются по-разному намеренно:
 * - `unavailable` — полноэкранное уведомление: работать с деревом больше
 *   нельзя, и главное, что должен понять пользователь, — заметки **не
 *   удалены** (FR-020), а несохранённые правки живы в памяти сессии (FR-035);
 * - `readOnly` — неблокирующее предупреждение: дерево и содержимое остаются
 *   доступными как обычно, отказывают только операции записи (FR-036), и
 *   отдельного заранее объявленного «режима только чтения» спека не вводит.
 */
export function AccessStateBanner({
  accessState,
  onReopenVault,
}: AccessStateBannerProps): React.JSX.Element | null {
  if (accessState === null) return null;

  switch (accessState) {
    case "readWrite":
      return null;
    case "readOnly":
      return (
        <div role="status" aria-live="polite">
          <p>
            Права на запись в vault отозваны. Заметки открываются и читаются как
            обычно, но сохранение, создание, переименование и удаление сейчас
            отклоняются.
          </p>
        </div>
      );
    case "unavailable":
      return (
        <div role="alertdialog" aria-modal="true" aria-label="Vault недоступен">
          <p>
            Vault стал недоступен — например, отключён диск или отозван доступ к
            папке. Это <strong>не</strong> означает, что заметки удалены.
          </p>
          <p>
            Несохранённые правки открытых заметок остаются в памяти, пока
            приложение работает: если закрыть его до восстановления доступа, они
            будут потеряны.
          </p>
          {onReopenVault && (
            <button type="button" onClick={onReopenVault}>
              Открыть vault заново
            </button>
          )}
        </div>
      );
    default: {
      const exhaustive: never = accessState;
      return exhaustive;
    }
  }
}
