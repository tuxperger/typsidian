import type { DiffLine } from "../wasm-core/typsidian_core.js";
import { useI18n, type Translate } from "../i18n/index.js";

export interface DiffViewProps {
  diff: DiffLine[];
}

function sideLabel(t: Translate, line: DiffLine): string {
  switch (line.side) {
    case "Same":
      return t("editor.diff.same");
    case "Local":
      return t("editor.diff.local");
    case "External":
      return t("editor.diff.external");
    default: {
      const exhaustive: never = line;
      return exhaustive;
    }
  }
}

function sideMarker(line: DiffLine): string {
  switch (line.side) {
    case "Same":
      return " ";
    case "Local":
      return "<";
    case "External":
      return ">";
    default: {
      const exhaustive: never = line;
      return exhaustive;
    }
  }
}

/**
 * Показ отличий между версией пользователя и версией на диске (T077, FR-024).
 * Сам diff вычисляет ядро (`conflictDiff`) — здесь только отрисовка: правило
 * «что считать одинаковой строкой» не должно существовать в двух местах
 * (принцип I).
 *
 * Просмотр отличий ничего не разрешает: после него пользователь возвращается к
 * тому же выбору из трёх вариантов (FR-024), поэтому у компонента нет ни одного
 * действия.
 */
export function DiffView({ diff }: DiffViewProps): React.JSX.Element {
  const { t } = useI18n();
  if (diff.length === 0) {
    return <p role="status">{t("editor.diff.none")}</p>;
  }
  return (
    <ol aria-label={t("editor.diff.label")}>
      {diff.map((line, index) => (
        <li
          // Строки не уникальны и могут повторяться, поэтому ключ — позиция в
          // уже готовом, неизменяемом списке отличий.
          key={`${index}:${line.side}`}
          data-side={line.side}
        >
          <span aria-hidden="true">{sideMarker(line)}</span>
          <span className="sr-only">{sideLabel(t, line)}: </span>
          <code>{line.text === "" ? " " : line.text}</code>
        </li>
      ))}
    </ol>
  );
}
