import { useI18n } from "../i18n/index.js";
import type { AccessState } from "../wasm-core/typsidian_core.js";

export interface AccessStateBannerProps {
  accessState: AccessState | null;
  /** FR-018: восстановление из `Unavailable` — только явным переоткрытием. */
  onReopenVault?: () => void;

  /**
   * Служебное место vault исчезло вместе с настройками уровня папки и
   * локальными шаблонами (фича 006, FR-024).
   *
   * Приходит фактом из ядра (`VaultEvent::VaultSettingsLost`), а не выводится
   * здесь: отличить «удалили» от «никогда не было» можно только зная, открывали
   * ли эту папку на этом устройстве, а список известных vault принадлежит
   * настройкам устройства, а не дереву.
   */
  settingsLost?: boolean;
  /**
   * Vault недоступен на запись, поэтому настройки уровня папки менять некуда
   * (фича 006, FR-025).
   *
   * Отдельно от `accessState === "readOnly"`: то сообщение — про заметки, это —
   * про настройки, и пользователю нужны разные слова.
   */
  settingsReadOnly?: boolean;
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
  settingsLost = false,
  settingsReadOnly = false,
}: AccessStateBannerProps): React.JSX.Element | null {
  const { t } = useI18n();
  // Сообщения о настройках уровня vault (фича 006) идут до разбора
  // `accessState`: при `readWrite` тот вернул бы `null`, и об утрате настроек
  // пользователь не узнал бы вовсе.
  const settingsNotice = settingsLost ? (
    <div role="status" aria-live="polite" className="ts-banner ts-banner--warn">
      <p>{t("tree.access.settingsLost")}</p>
    </div>
  ) : settingsReadOnly ? (
    <div role="status" aria-live="polite" className="ts-banner ts-banner--warn">
      <p>{t("tree.access.settingsReadOnly")}</p>
    </div>
  ) : null;

  if (accessState === null) return settingsNotice;

  switch (accessState) {
    case "readWrite":
      return settingsNotice;
    case "readOnly":
      return (
        <>
          <div role="status" aria-live="polite">
            <p>{t("tree.access.readOnly")}</p>
          </div>
          {settingsNotice}
        </>
      );
    case "unavailable":
      return (
        <div
          className="ts-dialog"
          role="alertdialog"
          aria-modal="true"
          aria-label={t("tree.access.unavailableTitle")}
        >
          <p>
            {t("tree.access.unavailableLead")}
            <strong>{t("tree.access.unavailableNot")}</strong>
            {t("tree.access.unavailableTail")}
          </p>
          <p>{t("tree.access.unsavedKept")}</p>
          {onReopenVault && (
            <button type="button" onClick={onReopenVault}>
              {t("tree.access.reopen")}
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
