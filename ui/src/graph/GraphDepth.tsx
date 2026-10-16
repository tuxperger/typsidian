import { useI18n } from "../i18n/index.js";

/** Выбор глубины локального графа 1–3 (макет G2, G3). */
export function GraphDepth({
  depth,
  onChange,
}: {
  depth: number;
  onChange: (depth: number) => void;
}): React.JSX.Element {
  const { t } = useI18n();
  return (
    <div className="ts-graph__depth" role="group" aria-label={t("graph.depth.label")}>
      <span aria-hidden="true">{t("graph.depth.label").toLowerCase()}</span>
      {[1, 2, 3].map((value) => (
        <button
          key={value}
          type="button"
          aria-pressed={depth === value}
          aria-label={t("graph.depth.value", { depth: value })}
          onClick={() => onChange(value)}
        >
          {value}
        </button>
      ))}
    </div>
  );
}
