/**
 * Роли подсветки Typst (фича 024, data-model.md).
 *
 * Номер роли — её индекс в `SYNTAX_ROLES`, он же номер `SyntaxRole` в ядре.
 * Порядок сверяет тест ядра `roles_match_frontend_table`: роль, добавленная
 * только здесь или только в ядре, роняет его, а не раскраску.
 */
export const SYNTAX_ROLES = [
  "Comment",
  "Punctuation",
  "Escape",
  "MarkupMarker",
  "HeadingMarker",
  "ListMarker",
  "ListTerm",
  "Link",
  "Raw",
  "Label",
  "Ref",
  "MathDelimiter",
  "MathOperator",
  "MathIdent",
  "Keyword",
  "Constant",
  "Operator",
  "Number",
  "String",
  "Function",
  "ArgName",
  "Variable",
  "Plain",
] as const;

export type SyntaxRole = (typeof SYNTAX_ROLES)[number];

/** Начертание от предков — биты старшего байта упакованной роли. */
const STYLE_FLAGS = [
  { bit: 1, className: "ts-syn-strong" },
  { bit: 1 << 1, className: "ts-syn-emph" },
  { bit: 1 << 2, className: "ts-syn-heading" },
] as const;

/** `MarkupMarker` → `ts-syn-markup-marker`. */
function roleClass(role: SyntaxRole): string {
  return `ts-syn-${role.replace(/[A-Z]/g, (letter, at: number) => (at === 0 ? "" : "-") + letter.toLowerCase())}`;
}

const ROLE_CLASSES: readonly string[] = SYNTAX_ROLES.map(roleClass);

/**
 * Классы фрагмента по упакованному значению `role | flags << 8`.
 * Неизвестная роль (ядро новее интерфейса) — без класса роли, но с начертанием.
 */
export function classesOf(packed: number): string {
  const role = ROLE_CLASSES[packed & 0xff];
  const flags = packed >>> 8;
  const classes = role === undefined ? [] : [role];
  for (const flag of STYLE_FLAGS) {
    if ((flags & flag.bit) !== 0) classes.push(flag.className);
  }
  return classes.join(" ");
}
