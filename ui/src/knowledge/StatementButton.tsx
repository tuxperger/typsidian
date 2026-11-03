"use client";

import type { StatementRef } from "../wasm-core/typsidian_core.js";

// Формулировка утверждения, которую можно выбрать: открывает его колонку.

export function StatementButton({
  statement,
  selected,
  onSelect,
}: {
  statement: StatementRef;
  selected: boolean;
  onSelect: (id: string) => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      className="ts-kg__stmt"
      aria-pressed={selected}
      onClick={() => onSelect(statement.id)}
    >
      {statement.text}
    </button>
  );
}
