import type { DiffLine } from "../wasm-core/typsidian_core.js";

export interface DiffViewProps {
  diff: DiffLine[];
}

function sideLabel(line: DiffLine): string {
  switch (line.side) {
    case "Same":
      return "без изменений";
    case "Local":
      return "ваша версия";
    case "External":
      return "версия на диске";
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
  if (diff.length === 0) {
    return <p role="status">Отличий нет.</p>;
  }
  return (
    <ol aria-label="Отличия между вашей версией и версией на диске">
      {diff.map((line, index) => (
        <li
          // Строки не уникальны и могут повторяться, поэтому ключ — позиция в
          // уже готовом, неизменяемом списке отличий.
          key={`${index}:${line.side}`}
          data-side={line.side}
        >
          <span aria-hidden="true">{sideMarker(line)}</span>
          <span className="sr-only">{sideLabel(line)}: </span>
          <code>{line.text === "" ? " " : line.text}</code>
        </li>
      ))}
    </ol>
  );
}
