// Палитра математических символов Typst (макет «Typsidian Вставка» →
// § Палитра математики).
//
// В исходник вставляется **имя**, а не глиф: `sum`, а не «∑». Так текст
// заметки остаётся ASCII — его одинаково видно в любом редакторе, он не
// зависит от шрифта и читается в diff. Глиф здесь нужен только для узнавания
// в сетке.
//
// Список неполный и полным не будет: у Typst символов больше тысячи. Здесь то,
// что набирают в технической заметке, — остальное пишется руками, как и
// раньше.

import type { Translate } from "../i18n/index.js";

export type SymbolCategory = "struct" | "greek" | "op" | "rel" | "arrow" | "accent";

export const SYMBOL_CATEGORIES: { id: SymbolCategory }[] = [
  { id: "struct" },
  { id: "greek" },
  { id: "op" },
  { id: "rel" },
  { id: "arrow" },
  { id: "accent" },
];

/** Название раздела палитры на языке интерфейса. */
export function symbolCategoryName(t: Translate, id: SymbolCategory): string {
  return t(`insert.symbols.category.${id}` as const);
}

export interface MathSymbol {
  category: SymbolCategory;
  /** Как символ выглядит на бумаге — подпись плитки, не вставляемый текст. */
  glyph: string;
  /** Имя Typst: и подпись под глифом, и строка поиска, и то, что вставляется. */
  name: string;
  /**
   * Шаблон для символов-функций: `frac` без аргументов не имеет смысла,
   * поэтому вставляется вместе со скобками и полями.
   */
  template?: string;
}

/** Столбцов в сетке — тем же числом навигация считает шаг ↑↓. */
export const SYMBOL_COLUMNS = 8;

export const MATH_SYMBOLS: MathSymbol[] = [
  { category: "struct", glyph: "a⁄b", name: "frac", template: "frac(${a}, ${b})" },
  { category: "struct", glyph: "√x", name: "sqrt", template: "sqrt(${x})" },
  { category: "struct", glyph: "∛x", name: "root", template: "root(${3}, ${x})" },
  { category: "struct", glyph: "[ ]", name: "mat", template: "mat(${1}, ${2}; ${3}, ${4})" },
  { category: "struct", glyph: "( )", name: "vec", template: "vec(${1}, ${2})" },
  { category: "struct", glyph: "{", name: "cases", template: "cases(${x = 1}, ${y = 2})" },
  { category: "struct", glyph: "|x|", name: "abs", template: "abs(${x})" },
  { category: "struct", glyph: "‖x‖", name: "norm", template: "norm(${x})" },
  { category: "struct", glyph: "lim", name: "lim", template: "lim_(${x -> 0})" },
  { category: "struct", glyph: "C(n,k)", name: "binom", template: "binom(${n}, ${k})" },

  { category: "greek", glyph: "α", name: "alpha" },
  { category: "greek", glyph: "β", name: "beta" },
  { category: "greek", glyph: "γ", name: "gamma" },
  { category: "greek", glyph: "δ", name: "delta" },
  { category: "greek", glyph: "ε", name: "epsilon" },
  { category: "greek", glyph: "θ", name: "theta" },
  { category: "greek", glyph: "λ", name: "lambda" },
  { category: "greek", glyph: "μ", name: "mu" },
  { category: "greek", glyph: "ν", name: "nu" },
  { category: "greek", glyph: "π", name: "pi" },
  { category: "greek", glyph: "ρ", name: "rho" },
  { category: "greek", glyph: "σ", name: "sigma" },
  { category: "greek", glyph: "τ", name: "tau" },
  { category: "greek", glyph: "φ", name: "phi" },
  { category: "greek", glyph: "ψ", name: "psi" },
  { category: "greek", glyph: "ω", name: "omega" },
  { category: "greek", glyph: "Γ", name: "Gamma" },
  { category: "greek", glyph: "Δ", name: "Delta" },
  { category: "greek", glyph: "Θ", name: "Theta" },
  { category: "greek", glyph: "Σ", name: "Sigma" },
  { category: "greek", glyph: "Φ", name: "Phi" },
  { category: "greek", glyph: "Ω", name: "Omega" },

  { category: "op", glyph: "±", name: "plus.minus" },
  { category: "op", glyph: "∓", name: "minus.plus" },
  { category: "op", glyph: "×", name: "times" },
  // Точка-оператор и точка-акцент — разные имена: `dot` ставит точку **над**
  // символом, умножение — это `dot.op`.
  { category: "op", glyph: "⋅", name: "dot.op" },
  { category: "op", glyph: "÷", name: "div" },
  { category: "op", glyph: "⊗", name: "times.circle" },
  { category: "op", glyph: "⊕", name: "plus.circle" },
  { category: "op", glyph: "∘", name: "compose" },
  { category: "op", glyph: "∂", name: "diff" },
  { category: "op", glyph: "∇", name: "nabla" },
  { category: "op", glyph: "∞", name: "infinity" },
  { category: "op", glyph: "∑", name: "sum" },
  { category: "op", glyph: "∏", name: "product" },
  { category: "op", glyph: "∫", name: "integral" },

  { category: "rel", glyph: "≠", name: "eq.not" },
  { category: "rel", glyph: "≤", name: "lt.eq" },
  { category: "rel", glyph: "≥", name: "gt.eq" },
  { category: "rel", glyph: "≈", name: "approx" },
  { category: "rel", glyph: "≡", name: "equiv" },
  { category: "rel", glyph: "∝", name: "prop" },
  { category: "rel", glyph: "∈", name: "in" },
  { category: "rel", glyph: "∉", name: "in.not" },
  { category: "rel", glyph: "⊂", name: "subset" },
  { category: "rel", glyph: "⊆", name: "subset.eq" },
  { category: "rel", glyph: "∀", name: "forall" },
  { category: "rel", glyph: "∃", name: "exists" },

  { category: "arrow", glyph: "→", name: "arrow.r" },
  { category: "arrow", glyph: "←", name: "arrow.l" },
  { category: "arrow", glyph: "↑", name: "arrow.t" },
  { category: "arrow", glyph: "↓", name: "arrow.b" },
  { category: "arrow", glyph: "⇒", name: "arrow.r.double" },
  { category: "arrow", glyph: "⇔", name: "arrow.l.r.double" },
  { category: "arrow", glyph: "↦", name: "arrow.r.bar" },
  { category: "arrow", glyph: "⟶", name: "arrow.r.long" },

  { category: "accent", glyph: "x̂", name: "hat", template: "hat(${x})" },
  { category: "accent", glyph: "x̃", name: "tilde", template: "tilde(${x})" },
  { category: "accent", glyph: "ẋ", name: "dot", template: "dot(${x})" },
  { category: "accent", glyph: "ẍ", name: "dot.double", template: "dot.double(${x})" },
  { category: "accent", glyph: "x⃗", name: "arrow", template: "arrow(${x})" },
  { category: "accent", glyph: "x̄", name: "overline", template: "overline(${x})" },
  { category: "accent", glyph: "x̲", name: "underline", template: "underline(${x})" },
  { category: "accent", glyph: "𝐱", name: "bold", template: "bold(${x})" },
  { category: "accent", glyph: "x", name: "upright", template: "upright(${x})" },
];

/** Что уйдёт в текст: у функций — шаблон с полями, у остальных — само имя. */
export function symbolTemplate(symbol: MathSymbol): string {
  return symbol.template ?? symbol.name;
}
