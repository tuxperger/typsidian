import type { IconName } from "../icons/Icons.js";
import type { MessageKey, Translate } from "../i18n/index.js";

// Каталог элементов Typst для меню вставки (макет «Typsidian Вставка» →
// § Каталог элементов).
//
// Это данные, а не знание о языке: разметка перечислена ровно та, что человек
// набирает руками чаще всего. Компилятор о списке не знает и знать не должен —
// проверяет вставленное он сам, при ближайшей сборке предпросмотра.
//
// Состав сверен со справочником https://typst.app/docs/reference для версии
// компилятора, объявленной в `packages/core/Cargo.toml` (0.15). Проверять при
// обновлении компилятора: в 0.14–0.15 появились `title`, `divider`, `curve`, и
// так же молча может исчезнуть что-то отсюда — меню предложит элемент, а сборка
// предпросмотра ответит ошибкой.
//
// Шаблоны — в обычных кавычках: `${…}` здесь поле сниппета (см. `snippet.ts`),
// а в шаблонной строке JS оно превратилось бы в подстановку. Внутри поля не
// бывает `}`: разбор шаблона закрывает поле первой же закрывающей скобкой.
//
// Название, описание и шаблон живут в каталогах переводов (`elements.<id>.*`,
// фича 010): заглушки полей попадают в заметку и должны быть на языке
// интерфейса. Здесь — только то, что от языка не зависит.

export type ElementCategory =
  | "struct"
  | "list"
  | "math"
  | "block"
  | "layout"
  | "draw"
  | "style"
  | "data"
  | "link"
  | "mark";

/** Порядок разделов в меню — от каркаса документа к оформлению строки. */
export const ELEMENT_CATEGORIES: { id: ElementCategory }[] = [
  { id: "struct" },
  { id: "list" },
  { id: "math" },
  { id: "block" },
  { id: "layout" },
  { id: "draw" },
  { id: "style" },
  { id: "data" },
  { id: "link" },
  { id: "mark" },
];

/** Название раздела на языке интерфейса. */
export function categoryName(t: Translate, id: ElementCategory): string {
  return t(`elements.category.${id}` as const);
}

/**
 * Имя элемента — то, под которым у него есть тексты в каталоге. Выводится из
 * самого каталога: элемент без перевода не соберётся.
 */
export type ElementId =
  Extract<MessageKey, `elements.${string}.name`> extends `elements.${infer Id}.name`
    ? Id
    : never;

export interface TypstElement {
  id: ElementId;
  category: ElementCategory;
  icon: IconName;
  /**
   * Имена из документации Typst для поиска — там, где в шаблоне их нет:
   * маркированный список набирается как `-`, и по запросу «list» он иначе не
   * находится.
   */
  keywords?: string;
  /** Блочная разметка начинает строку. */
  block?: boolean;
  /**
   * Вместо вставки открывает палитру математических символов: имён у них
   * полторы сотни, и списком элементов они не показываются.
   */
  opensSymbols?: boolean;
}

/** Элемент вместе с текстами на языке интерфейса. */
export interface LocalizedElement extends TypstElement {
  name: string;
  /**
   * Одна строка о том, что элемент делает, — подпись над кодом в предпросмотре.
   * Обязательна: каталог на сотню имён без объяснений читается как справочник,
   * который уже есть на typst.app, а не как меню.
   */
  summary: string;
  /** Шаблон сниппета; `${…}` — поле для заполнения. */
  template: string;
}

export const TYPST_ELEMENTS: TypstElement[] = [
  {
    id: "heading-1",
    category: "struct",
    icon: "heading",
    keywords: "heading",
    block: true,
  },
  {
    id: "heading-2",
    category: "struct",
    icon: "heading",
    keywords: "heading",
    block: true,
  },
  {
    id: "heading-3",
    category: "struct",
    icon: "heading",
    keywords: "heading",
    block: true,
  },
  {
    id: "heading-4",
    category: "struct",
    icon: "heading",
    keywords: "heading",
    block: true,
  },
  {
    id: "title",
    category: "struct",
    icon: "heading",
    // Вместе с `#set document`: `#title()` без заданного названия — это
    // ошибка сборки, и меню не должно предлагать заведомо красный предпросмотр.
    block: true,
  },
  {
    id: "outline",
    category: "struct",
    icon: "list",
    block: true,
  },
  {
    id: "divider",
    category: "struct",
    icon: "page",
    block: true,
  },
  {
    id: "pagebreak",
    category: "struct",
    icon: "page",
    block: true,
  },
  {
    id: "parbreak",
    category: "struct",
    icon: "text",
    block: true,
  },

  {
    id: "list-bullet",
    category: "list",
    icon: "list",
    keywords: "list bullet",
    block: true,
  },
  {
    id: "list-numbered",
    category: "list",
    icon: "listnum",
    keywords: "enum numbered",
    block: true,
  },
  {
    id: "list-nested",
    category: "list",
    icon: "list",
    keywords: "list nested",
    block: true,
  },
  {
    id: "list-terms",
    category: "list",
    icon: "list",
    keywords: "terms",
    block: true,
  },
  {
    id: "enum-numbering",
    category: "list",
    icon: "listnum",
    keywords: "set enum numbering",
    block: true,
  },
  {
    id: "list-marker",
    category: "list",
    icon: "list",
    keywords: "set list marker",
    block: true,
  },

  {
    id: "math-inline",
    category: "math",
    icon: "sigma",
    keywords: "equation inline",
  },
  {
    id: "math-block",
    category: "math",
    icon: "sigma",
    keywords: "equation block",
    block: true,
  },
  {
    id: "math-numbering",
    category: "math",
    icon: "sigma",
    keywords: "set math equation numbering",
    block: true,
  },
  {
    id: "math-frac",
    category: "math",
    icon: "sigma",
    keywords: "frac",
  },
  {
    id: "math-sqrt",
    category: "math",
    icon: "sigma",
    keywords: "sqrt root",
  },
  {
    id: "math-sum",
    category: "math",
    icon: "sigma",
    keywords: "sum",
  },
  {
    id: "math-integral",
    category: "math",
    icon: "sigma",
    keywords: "integral",
  },
  {
    id: "math-matrix",
    category: "math",
    icon: "sigma",
    keywords: "mat matrix",
  },
  {
    id: "math-vector",
    category: "math",
    icon: "sigma",
    keywords: "vec vector",
  },
  {
    id: "math-cases",
    category: "math",
    icon: "sigma",
    keywords: "cases",
  },
  {
    id: "math-binom",
    category: "math",
    icon: "sigma",
    keywords: "binom",
  },
  {
    id: "math-attach",
    category: "math",
    icon: "sigma",
    keywords: "attach",
  },
  {
    id: "math-accent",
    category: "math",
    icon: "sigma",
    keywords: "accent hat arrow",
  },
  {
    id: "math-underbrace",
    category: "math",
    icon: "sigma",
    keywords: "underbrace",
    // Подпись в кавычках: без них `сумма` в формуле — это переменная из шести
    // букв, набранная курсивом, а не слово.
  },
  {
    id: "math-cancel",
    category: "math",
    icon: "sigma",
    keywords: "cancel",
  },
  {
    id: "math-lr",
    category: "math",
    icon: "sigma",
    keywords: "lr left right",
  },
  {
    id: "math-op",
    category: "math",
    icon: "sigma",
    keywords: "op",
  },
  {
    id: "math-bb",
    category: "math",
    icon: "sigma",
    keywords: "bb blackboard",
  },
  {
    id: "math-symbols",
    category: "math",
    icon: "sigma",
    opensSymbols: true,
  },

  {
    id: "figure",
    category: "block",
    icon: "image",
    keywords: "figure image",
    block: true,
  },
  {
    id: "figure-table",
    category: "block",
    icon: "table",
    keywords: "figure table caption",
    block: true,
  },
  {
    id: "table",
    category: "block",
    icon: "table",
    keywords: "table",
    block: true,
  },
  {
    id: "table-header",
    category: "block",
    icon: "table",
    keywords: "table header",
    block: true,
  },
  {
    id: "code-block",
    category: "block",
    icon: "code",
    keywords: "raw code block",
    // Внутри поля не бывает `}`: разбор шаблона (`snippet.ts`) закрывает поле
    // первой же закрывающей скобкой, и `fn main() {}` в значении по умолчанию
    // разорвал бы его пополам.
    block: true,
  },
  {
    id: "quote",
    category: "block",
    icon: "quote",
    keywords: "quote",
    block: true,
  },
  {
    id: "block-fill",
    category: "block",
    icon: "page",
    keywords: "block fill inset",
    block: true,
  },

  {
    id: "page-setup",
    category: "layout",
    icon: "page",
    keywords: "set page paper margin numbering",
    block: true,
  },
  {
    id: "page-header",
    category: "layout",
    icon: "page",
    keywords: "set page header footer",
    block: true,
  },
  {
    id: "columns",
    category: "layout",
    icon: "layout",
    keywords: "columns gutter",
    block: true,
  },
  {
    id: "colbreak",
    category: "layout",
    icon: "layout",
    keywords: "colbreak",
    block: true,
  },
  {
    id: "grid",
    category: "layout",
    icon: "layout",
    keywords: "grid columns gutter",
    block: true,
  },
  {
    id: "stack",
    category: "layout",
    icon: "layout",
    keywords: "stack dir spacing",
    block: true,
  },
  {
    id: "align",
    category: "layout",
    icon: "layout",
    keywords: "align",
    block: true,
  },
  {
    id: "pad",
    category: "layout",
    icon: "layout",
    keywords: "pad",
    block: true,
  },
  {
    id: "place",
    category: "layout",
    icon: "layout",
    keywords: "place float dx dy",
    block: true,
  },
  {
    id: "box",
    category: "layout",
    icon: "layout",
    keywords: "box",
  },
  {
    id: "hspace",
    category: "layout",
    icon: "layout",
    keywords: "h space",
  },
  {
    id: "vspace",
    category: "layout",
    icon: "layout",
    keywords: "v space",
    block: true,
  },
  {
    id: "repeat",
    category: "layout",
    icon: "layout",
    keywords: "repeat",
  },
  {
    id: "hide",
    category: "layout",
    icon: "layout",
    keywords: "hide",
  },
  {
    id: "move",
    category: "layout",
    icon: "layout",
    keywords: "move dx dy",
  },
  {
    id: "rotate",
    category: "layout",
    icon: "layout",
    keywords: "rotate",
  },
  {
    id: "scale",
    category: "layout",
    icon: "layout",
    keywords: "scale",
  },

  {
    id: "image",
    category: "draw",
    icon: "image",
    keywords: "image",
    block: true,
  },
  {
    id: "rect",
    category: "draw",
    icon: "shape",
    keywords: "rect",
  },
  {
    id: "square",
    category: "draw",
    icon: "shape",
    keywords: "square",
  },
  {
    id: "circle",
    category: "draw",
    icon: "shape",
    keywords: "circle",
  },
  {
    id: "ellipse",
    category: "draw",
    icon: "shape",
    keywords: "ellipse",
  },
  {
    id: "line",
    category: "draw",
    icon: "shape",
    keywords: "line stroke",
    block: true,
  },
  {
    id: "polygon",
    category: "draw",
    icon: "shape",
    keywords: "polygon regular",
  },
  {
    id: "gradient",
    category: "draw",
    icon: "shape",
    keywords: "gradient linear fill",
    block: true,
  },

  {
    id: "set-text",
    category: "style",
    icon: "braces",
    keywords: "set text font size lang",
    block: true,
  },
  {
    id: "set-par",
    category: "style",
    icon: "braces",
    keywords: "set par justify leading indent",
    block: true,
  },
  {
    id: "set-heading",
    category: "style",
    icon: "braces",
    keywords: "set heading numbering",
    block: true,
  },
  {
    id: "set-document",
    category: "style",
    icon: "braces",
    keywords: "set document title author",
    block: true,
  },
  {
    id: "show-set",
    category: "style",
    icon: "braces",
    keywords: "show set where",
    block: true,
  },
  {
    id: "show-function",
    category: "style",
    icon: "braces",
    keywords: "show function it",
    block: true,
  },
  {
    id: "show-regex",
    category: "style",
    icon: "braces",
    keywords: "show regex",
    block: true,
  },
  {
    id: "show-template",
    category: "style",
    icon: "braces",
    keywords: "show with template",
    block: true,
  },
  {
    id: "let-value",
    category: "style",
    icon: "braces",
    keywords: "let",
    block: true,
  },
  {
    id: "let-function",
    category: "style",
    icon: "braces",
    keywords: "let function",
    block: true,
  },
  {
    id: "import-package",
    category: "style",
    icon: "braces",
    keywords: "import preview package",
    block: true,
  },
  {
    id: "import-file",
    category: "style",
    icon: "braces",
    keywords: "import",
    block: true,
  },
  {
    id: "include-file",
    category: "style",
    icon: "braces",
    keywords: "include",
    block: true,
  },
  {
    id: "context",
    category: "style",
    icon: "braces",
    keywords: "context",
  },

  {
    id: "data-csv",
    category: "data",
    icon: "data",
    keywords: "csv",
    block: true,
  },
  {
    id: "data-json",
    category: "data",
    icon: "data",
    keywords: "json",
    block: true,
  },
  {
    id: "data-yaml",
    category: "data",
    icon: "data",
    keywords: "yaml",
    block: true,
  },
  {
    id: "data-read",
    category: "data",
    icon: "data",
    keywords: "read",
  },
  {
    id: "data-table",
    category: "data",
    icon: "data",
    keywords: "table csv flatten",
    block: true,
  },
  {
    id: "counter-display",
    category: "data",
    icon: "data",
    keywords: "counter display context",
  },
  {
    id: "counter-update",
    category: "data",
    icon: "data",
    keywords: "counter update",
    block: true,
  },
  {
    id: "numbering",
    category: "data",
    icon: "data",
    keywords: "numbering",
  },
  // `#datetime.today()` в меню нет намеренно: предпросмотр собирается без
  // текущей даты (`preview/world.rs`, `today()` → `None`), чтобы результат
  // сборки не менялся сам по себе. Снипет с ней всегда давал бы ошибку
  // «unable to get the current date» — меню не предлагает заведомо красное.
  {
    id: "lorem",
    category: "data",
    icon: "data",
    keywords: "lorem",
  },

  {
    id: "wiki-link",
    category: "link",
    icon: "link",
  },
  {
    id: "cross-ref",
    category: "link",
    icon: "tag",
    keywords: "ref",
  },
  {
    id: "label",
    category: "link",
    icon: "tag",
    keywords: "label",
  },
  {
    id: "external-link",
    category: "link",
    icon: "link",
    keywords: "link url",
  },
  {
    id: "cite",
    category: "link",
    icon: "quote",
    keywords: "cite",
  },
  {
    id: "cite-supplement",
    category: "link",
    icon: "quote",
    keywords: "cite supplement",
  },
  {
    id: "bibliography",
    category: "link",
    icon: "quote",
    keywords: "bibliography",
    block: true,
  },
  {
    id: "footnote",
    category: "link",
    icon: "text",
    keywords: "footnote",
  },

  {
    id: "bold",
    category: "mark",
    icon: "text",
    keywords: "strong bold",
  },
  {
    id: "italic",
    category: "mark",
    icon: "text",
    keywords: "emph italic",
  },
  {
    id: "raw",
    category: "mark",
    icon: "code",
    keywords: "raw code",
  },
  {
    id: "strike",
    category: "mark",
    icon: "text",
    keywords: "strike",
  },
  {
    id: "underline",
    category: "mark",
    icon: "text",
    keywords: "underline",
  },
  {
    id: "overline",
    category: "mark",
    icon: "text",
    keywords: "overline",
  },
  {
    id: "highlight",
    category: "mark",
    icon: "text",
    keywords: "highlight",
  },
  {
    id: "smallcaps",
    category: "mark",
    icon: "text",
    keywords: "smallcaps",
  },
  {
    id: "super",
    category: "mark",
    icon: "text",
    keywords: "super",
  },
  {
    id: "sub",
    category: "mark",
    icon: "text",
    keywords: "sub",
  },
  {
    id: "upper",
    category: "mark",
    icon: "text",
    keywords: "upper",
  },
  {
    id: "text-style",
    category: "mark",
    icon: "text",
    keywords: "text fill weight",
  },
  {
    id: "linebreak",
    category: "mark",
    icon: "text",
    keywords: "linebreak",
  },
  {
    id: "comment",
    category: "mark",
    icon: "text",
    block: true,
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

/**
 * Элемент каталога по имени. `undefined` — такого имени нет: панель вставки и
 * прочие «короткие пути» ссылаются на элементы именами, и опечатка обязана
 * оборачиваться отсутствующей кнопкой, а не вставкой чужого шаблона.
 */
export function elementById(t: Translate, id: string): LocalizedElement | undefined {
  const element = TYPST_ELEMENTS.find((candidate) => candidate.id === id);
  return element === undefined ? undefined : localizeElement(t, element);
}

/** Тексты элемента на языке интерфейса. */
export function localizeElement(t: Translate, element: TypstElement): LocalizedElement {
  return {
    ...element,
    name: t(`elements.${element.id}.name` as const),
    summary: t(`elements.${element.id}.summary` as const),
    template: t(`elements.${element.id}.template` as const),
  };
}

/** Весь каталог на языке интерфейса. */
export function localizedElements(t: Translate): LocalizedElement[] {
  return TYPST_ELEMENTS.map((element) => localizeElement(t, element));
}

/**
 * Подходит ли элемент под строку поиска: по названию, коду и именам Typst.
 * Имена Typst (`keywords`) английские, поэтому элемент находится по ним на
 * любом языке интерфейса.
 */
export function matchesQuery(element: LocalizedElement, query: string): boolean {
  if (query === "") return true;
  return (
    element.name.toLowerCase().includes(query) ||
    element.template.toLowerCase().includes(query) ||
    element.summary.toLowerCase().includes(query) ||
    (element.keywords?.toLowerCase().includes(query) ?? false)
  );
}
