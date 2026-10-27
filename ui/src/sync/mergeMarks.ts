import type { LineOrigin } from "../wasm-core/typsidian_core.js";

/**
 * Нецветовой признак стороны (FR-005): тот же знак, что у сравнения версий
 * заметки (`editor/DiffView.tsx`), — `<` своя, `>` их. Это значки, а не текст:
 * их расшифровывает легенда редактора слияния.
 */
export function originMark(origin: LineOrigin): string {
  switch (origin) {
    case "Same":
      return "";
    case "Ours":
      return "<"; // i18n-ignore: знак стороны
    case "Theirs":
      return ">"; // i18n-ignore: знак стороны
    case "Both":
      return "±"; // i18n-ignore: знак стороны
    case "Manual":
      return "✎"; // i18n-ignore: знак стороны
    default: {
      const exhaustive: never = origin;
      return exhaustive;
    }
  }
}

/** Класс строки: цвет стороны задают стили `ts-merge-line--*`. */
export function originClass(origin: LineOrigin): string {
  return `ts-merge-line--${origin.toLowerCase()}`;
}
