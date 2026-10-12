import { useI18n } from "../i18n/index.js";
import { Icon } from "../icons/Icons.js";
import type { DragOver, DragSource } from "./useTreeDrag.js";

export interface TreeDragGhostProps {
  source: DragSource | null;
  over: DragOver;
  /** Положение ставит сам жест, мимо React — см. `useTreeDrag`. */
  ghostRef: React.RefObject<HTMLDivElement | null>;
  /** Как назвать корень vault в объявлении. */
  rootName: string;
}

/**
 * Ярлык у указателя и объявления жеста для читалки экрана (фича 023, FR-010,
 * FR-026). На сенсорном экране ярлык выглядит «поднятой» строкой (макет,
 * экран «Сенсорный экран»).
 *
 * Область объявлений смонтирована всегда: `aria-live`, появившийся вместе с
 * текстом, читалки пропускают.
 */
export function TreeDragGhost({
  source,
  over,
  ghostRef,
  rootName,
}: TreeDragGhostProps): React.JSX.Element {
  const { t } = useI18n();

  let announcement = "";
  if (source !== null) {
    switch (over.kind) {
      case "folder":
        announcement = t("tree.move.a11y.over", { folder: over.folder });
        break;
      case "root":
        announcement = `${t("tree.move.a11y.overRoot")} (${rootName})`;
        break;
      case "denied":
        announcement = t("tree.move.a11y.denied");
        break;
      default:
        announcement = t("tree.move.a11y.grabbed", { name: source.name });
    }
  }

  return (
    <>
      <span className="ts-tree-live" aria-live="polite">
        {announcement}
      </span>
      {source !== null && (
        <div
          ref={ghostRef}
          className={source.touch ? "ts-tree-drag ts-tree-drag--lifted" : "ts-tree-drag"}
          aria-hidden="true"
        >
          <Icon
            name={source.kind === "folder" ? "folder" : source.kind === "board" ? "board" : "file"}
            size={13}
          />
          <span className="ts-tree-drag__name">{source.name}</span>
          {source.count !== null && (
            <span className="ts-tree-drag__count">
              {t("tree.move.filesCount", { count: source.count })}
            </span>
          )}
        </div>
      )}
    </>
  );
}
