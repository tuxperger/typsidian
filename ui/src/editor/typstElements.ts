import type { IconName } from "../icons/Icons.js";

// Каталог элементов Typst для меню вставки (макет «Typsidian Вставка» →
// § Каталог элементов).
//
// Это данные, а не знание о языке: разметка перечислена ровно та, что человек
// набирает руками чаще всего. Компилятор о списке не знает и знать не должен —
// проверяет вставленное он сам, при ближайшей сборке предпросмотра.
//
// Шаблоны — в обычных кавычках: `${…}` здесь поле сниппета (см. `snippet.ts`),
// а в шаблонной строке JS оно превратилось бы в подстановку.

export type ElementCategory = "struct" | "list" | "math" | "block" | "link" | "mark";

/** Порядок разделов в меню — от каркаса документа к оформлению строки. */
export const ELEMENT_CATEGORIES: { id: ElementCategory; name: string }[] = [
  { id: "struct", name: "Структура" },
  { id: "list", name: "Списки" },
  { id: "math", name: "Математика" },
  { id: "block", name: "Блоки" },
  { id: "link", name: "Ссылки" },
  { id: "mark", name: "Разметка" },
];

export interface TypstElement {
  id: string;
  category: ElementCategory;
  name: string;
  icon: IconName;
  /** Шаблон сниппета; `${…}` — поле для заполнения. */
  template: string;
  /** Блочная разметка начинает строку. */
  block?: boolean;
  /**
   * Вместо вставки открывает палитру математических символов: имён у них
   * полторы сотни, и списком элементов они не показываются.
   */
  opensSymbols?: boolean;
}

export const TYPST_ELEMENTS: TypstElement[] = [
  {
    id: "heading-1",
    category: "struct",
    name: "Заголовок 1",
    icon: "heading",
    template: "= ${Заголовок}",
    block: true,
  },
  {
    id: "heading-2",
    category: "struct",
    name: "Заголовок 2",
    icon: "heading",
    template: "== ${Подзаголовок}",
    block: true,
  },
  {
    id: "heading-3",
    category: "struct",
    name: "Заголовок 3",
    icon: "heading",
    template: "=== ${Пункт}",
    block: true,
  },
  {
    id: "outline",
    category: "struct",
    name: "Оглавление",
    icon: "list",
    template: "#outline(depth: ${2})",
    block: true,
  },
  {
    id: "pagebreak",
    category: "struct",
    name: "Разрыв страницы",
    icon: "page",
    template: "#pagebreak()",
    block: true,
  },

  {
    id: "list-bullet",
    category: "list",
    name: "Маркированный список",
    icon: "list",
    template: "- ${Первый пункт}\n- ${Второй пункт}",
    block: true,
  },
  {
    id: "list-numbered",
    category: "list",
    name: "Нумерованный список",
    icon: "listnum",
    template: "+ ${Первый шаг}\n+ ${Второй шаг}",
    block: true,
  },
  {
    id: "list-terms",
    category: "list",
    name: "Список терминов",
    icon: "list",
    template: "/ ${Термин}: ${определение}",
    block: true,
  },

  {
    id: "math-inline",
    category: "math",
    name: "Формула в строке",
    icon: "sigma",
    template: "$${x^2}$",
  },
  {
    id: "math-block",
    category: "math",
    name: "Формула блоком",
    icon: "sigma",
    template: "$ ${T(x) = T_0 + Delta T} $",
    block: true,
  },
  {
    id: "math-frac",
    category: "math",
    name: "Дробь",
    icon: "sigma",
    template: "frac(${a}, ${b})",
  },
  {
    id: "math-sqrt",
    category: "math",
    name: "Корень",
    icon: "sigma",
    template: "sqrt(${x})",
  },
  {
    id: "math-sum",
    category: "math",
    name: "Сумма",
    icon: "sigma",
    template: "sum_(${i=1})^${n}",
  },
  {
    id: "math-integral",
    category: "math",
    name: "Интеграл",
    icon: "sigma",
    template: "integral_${a}^${b}",
  },
  {
    id: "math-matrix",
    category: "math",
    name: "Матрица",
    icon: "sigma",
    template: "mat(${1}, ${2}; ${3}, ${4})",
  },
  {
    id: "math-cases",
    category: "math",
    name: "Система уравнений",
    icon: "sigma",
    template: "cases(${x = 1}, ${y = 2})",
  },
  {
    id: "math-symbols",
    category: "math",
    name: "Символы и структуры…",
    icon: "sigma",
    template: "",
    opensSymbols: true,
  },

  {
    id: "figure",
    category: "block",
    name: "Рисунок",
    icon: "image",
    template:
      '#figure(\n  image("${data/график.png}", width: ${80%}),\n  caption: [${Подпись}],\n) <fig:${label}>',
    block: true,
  },
  {
    id: "table",
    category: "block",
    name: "Таблица",
    icon: "table",
    template: "#table(\n  columns: ${3},\n  [${A}], [${B}], [${C}],\n)",
    block: true,
  },
  {
    id: "code-block",
    category: "block",
    name: "Кодовый блок",
    icon: "code",
    // Внутри поля не бывает `}`: разбор шаблона (`snippet.ts`) закрывает поле
    // первой же закрывающей скобкой, и `fn main() {}` в значении по умолчанию
    // разорвал бы его пополам.
    template: "```${rust}\n${код}\n```",
    block: true,
  },
  {
    id: "quote",
    category: "block",
    name: "Цитата",
    icon: "quote",
    template: "#quote(block: true, attribution: [${Смирнов, 2023}])[\n  ${Текст цитаты.}\n]",
    block: true,
  },
  {
    id: "block-fill",
    category: "block",
    name: "Выделенный блок",
    icon: "page",
    template: "#block(fill: luma(240), inset: 8pt)[\n  ${Важное замечание}\n]",
    block: true,
  },

  {
    id: "wiki-link",
    category: "link",
    name: "Вики-ссылка",
    icon: "link",
    template: "[[${Методология}]]",
  },
  {
    id: "cross-ref",
    category: "link",
    name: "Перекрёстная ссылка",
    icon: "tag",
    template: "@${fig:thermogram}",
  },
  {
    id: "label",
    category: "link",
    name: "Метка",
    icon: "tag",
    template: "<${sec:results}>",
  },
  {
    id: "external-link",
    category: "link",
    name: "Внешняя ссылка",
    icon: "link",
    template: '#link("${https://typst.app}")[${документация}]',
  },
  {
    id: "cite",
    category: "link",
    name: "Библиографическая ссылка",
    icon: "quote",
    template: "#cite(<${smirnov2023}>)",
  },
  {
    id: "footnote",
    category: "link",
    name: "Сноска",
    icon: "text",
    template: "#footnote[${Примечание}]",
  },

  {
    id: "bold",
    category: "mark",
    name: "Жирный",
    icon: "text",
    template: "*${текст}*",
  },
  {
    id: "italic",
    category: "mark",
    name: "Курсив",
    icon: "text",
    template: "_${текст}_",
  },
  {
    id: "raw",
    category: "mark",
    name: "Моноширинный",
    icon: "code",
    template: "`${код}`",
  },
  {
    id: "strike",
    category: "mark",
    name: "Зачёркнутый",
    icon: "text",
    template: "#strike[${текст}]",
  },
  {
    id: "super",
    category: "mark",
    name: "Верхний индекс",
    icon: "text",
    template: "#super[${2}]",
  },
];

/**
 * Первая строка шаблона без разметки полей — подсказка справа в списке.
 *
 * Показывается именно код, а не сочетание клавиш: клавиш у элементов нет, а
 * код отвечает на вопрос «что именно окажется в файле» — тот самый, ради
 * которого меню и открывают.
 */
export function snippetPreview(template: string, limit = 18): string {
  const line = template.replace(/\$\{([^}]*)\}/g, "$1").split("\n")[0] ?? "";
  return line.length > limit ? `${line.slice(0, limit - 1)}…` : line;
}

/** Подходит ли элемент под строку поиска: по названию и по самому коду. */
export function matchesQuery(element: TypstElement, query: string): boolean {
  if (query === "") return true;
  return (
    element.name.toLowerCase().includes(query) ||
    element.template.toLowerCase().includes(query)
  );
}
