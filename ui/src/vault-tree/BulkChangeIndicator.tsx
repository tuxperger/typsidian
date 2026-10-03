import { useI18n } from "../i18n/index.js";

export interface BulkChangeIndicatorProps {
  /**
   * Идёт ли сейчас массовое изменение. Считать нужно именно вложенностью
   * `BulkChangeStarted`/`BulkChangeSettled`, а не булевым флагом: пачки могут
   * приходить подряд (например, `git checkout` порождает несколько циклов
   * дебаунсера), и одиночный `Settled` от первой пачки погасил бы индикатор
   * посреди второй. См. `useBulkChangeIndicator` ниже.
   */
  active: boolean;
}

/**
 * Единый индикатор массового внешнего изменения (T062, US3 AS5, FR-019):
 * заменяет собой шторм отдельных уведомлений на каждую изменённую заметку.
 * Само дерево при этом продолжает обновляться обычными событиями — индикатор
 * ничего не блокирует, он только объясняет, почему список сейчас «шевелится».
 */
export function BulkChangeIndicator({
  active,
}: BulkChangeIndicatorProps): React.JSX.Element | null {
  const { t } = useI18n();
  if (!active) return null;
  return (
    <div role="status" aria-live="polite">
      <p>{t("tree.bulkChange")}</p>
    </div>
  );
}

/**
 * Счётчик вложенности пачек массовых изменений. Возвращает функции, которые
 * подаются в `useVaultEvents` как `onBulkChangeStarted`/`onBulkChangeSettled`.
 * Выделен отдельно от компонента, потому что состояние принадлежит владельцу
 * потока событий, а не отрисовке.
 */
export function bulkChangeReducer(
  depth: number,
  event: "started" | "settled",
): number {
  if (event === "started") return depth + 1;
  return Math.max(0, depth - 1);
}
