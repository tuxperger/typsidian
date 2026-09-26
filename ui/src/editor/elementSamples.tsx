"use client";

import type { TypstElement } from "./typstElements.js";

// Мини-образцы для третьей колонки меню вставки (макет «Typsidian Вставка» →
// § Меню вставки: «сверху — мини-бумага с тем, как это скомпилируется»).
//
// # Это образец, а не отрисовка
//
// Настоящая отрисовка означала бы вызов компилятора на каждое движение по
// списку: он собирает документ десятки-сотни миллисекунд и делит очередь с
// колонкой предпросмотра, а меню обязано отвечать мгновенно. Поэтому здесь
// схема формы — «заголовок крупнее текста», «подпись под рамкой», «цитата с
// линейкой слева», — и подписана она в интерфейсе словом «образец», чтобы не
// притворяться готовой страницей (handoff прямо допускает статические образцы).
//
// # Чего здесь нет намеренно
//
// У элементов, которые меняют настройки документа (`#set`, `#show`, `#let`,
// импорты, счётчики, чтение данных), местного вида нет: их действие
// проявляется в другом месте документа и зависит от остального текста.
// Для них образца нет вовсе — колонка показывает только код. Нарисовать им
// «пример» значило бы выдумать результат, которого никто не обещал.

export function ElementSample({
  element,
}: {
  element: TypstElement;
}): React.JSX.Element | null {
  const byId = SAMPLES[element.id];
  if (byId) return byId;
  return FALLBACK[element.category] ?? null;
}

/** Строка «бумаги» — серая полоса вместо текста, как в макете. */
function Line({ width = "100%", tone = "text" }: { width?: string; tone?: "text" | "faint" }) {
  return <span className={`ts-sample__line ts-sample__line--${tone}`} style={{ width }} />;
}

function Para() {
  return (
    <span className="ts-sample__para">
      <Line />
      <Line width="88%" />
      <Line width="64%" />
    </span>
  );
}

function Heading({ size, text }: { size: number; text: string }) {
  return (
    <span className="ts-sample__heading" style={{ fontSize: `${size}px` }}>
      {text}
    </span>
  );
}

function Math({ children }: { children: React.ReactNode }) {
  return <span className="ts-sample__math">{children}</span>;
}

function Cells({ columns, rows, head }: { columns: number; rows: number; head?: boolean }) {
  const cells: React.JSX.Element[] = [];
  for (let row = 0; row < rows; row += 1) {
    for (let column = 0; column < columns; column += 1) {
      cells.push(
        <span
          key={`${row}-${column}`}
          className={`ts-sample__cell${head === true && row === 0 ? " ts-sample__cell--head" : ""}`}
        />,
      );
    }
  }
  return (
    <span
      className="ts-sample__grid"
      style={{ gridTemplateColumns: `repeat(${columns}, 1fr)` }}
    >
      {cells}
    </span>
  );
}

/** Рисунок: рамка с меткой вместо картинки и подпись курсивом под ней. */
function Figure({ caption = "Рис. 1. Подпись" }: { caption?: string }) {
  return (
    <>
      <span className="ts-sample__box">
        <span className="ts-sample__box-mark" />
      </span>
      <span className="ts-sample__caption">{caption}</span>
    </>
  );
}

function Bullets({ marker }: { marker: (index: number) => string }) {
  return (
    <span className="ts-sample__list">
      {[0, 1, 2].map((index) => (
        <span key={index} className="ts-sample__item">
          <span className="ts-sample__marker">{marker(index)}</span>
          <Line width={index === 2 ? "58%" : "80%"} />
        </span>
      ))}
    </span>
  );
}

/** Образцы, у которых вид свой. Остальные добираются по категории ниже. */
const SAMPLES: Record<string, React.JSX.Element> = {
  "heading-1": (
    <>
      <Heading size={13} text="Раздел" />
      <Para />
    </>
  ),
  "heading-2": (
    <>
      <Heading size={11} text="Подраздел" />
      <Para />
    </>
  ),
  "heading-3": (
    <>
      <Heading size={10} text="Пункт" />
      <Para />
    </>
  ),
  "heading-4": (
    <>
      <Heading size={9} text="Подпункт" />
      <Para />
    </>
  ),
  title: (
    <>
      <span className="ts-sample__title">Название работы</span>
      <span className="ts-sample__byline">А. Смирнов</span>
      <Para />
    </>
  ),
  outline: (
    <>
      <Heading size={10} text="Содержание" />
      <span className="ts-sample__toc">
        <span>
          Введение <i /> 3
        </span>
        <span>
          Методика <i /> 7
        </span>
      </span>
    </>
  ),
  divider: (
    <>
      <Line width="70%" />
      <span className="ts-sample__rule" />
      <Line width="84%" />
    </>
  ),
  pagebreak: (
    <span className="ts-sample__pages">
      <span className="ts-sample__page">
        <Line width="80%" />
        <Line width="60%" />
      </span>
      <span className="ts-sample__page ts-sample__page--next">
        <Line width="70%" />
      </span>
    </span>
  ),
  parbreak: (
    <>
      <Para />
      <span className="ts-sample__gap" />
      <Line width="76%" />
    </>
  ),

  "list-bullet": <Bullets marker={() => "•"} />,
  "list-numbered": <Bullets marker={(index) => `${index + 1}.`} />,
  "list-nested": (
    <span className="ts-sample__list">
      <span className="ts-sample__item">
        <span className="ts-sample__marker">•</span>
        <Line width="76%" />
      </span>
      <span className="ts-sample__item ts-sample__item--deep">
        <span className="ts-sample__marker">◦</span>
        <Line width="60%" />
      </span>
      <span className="ts-sample__item ts-sample__item--deep">
        <span className="ts-sample__marker">◦</span>
        <Line width="52%" />
      </span>
    </span>
  ),
  "list-terms": (
    <span className="ts-sample__list">
      <span className="ts-sample__term">Термин</span>
      <Line width="84%" />
      <span className="ts-sample__term">Второй</span>
      <Line width="66%" />
    </span>
  ),
  "enum-numbering": <Bullets marker={(index) => `${String.fromCharCode(97 + index)})`} />,
  "list-marker": <Bullets marker={() => "—"} />,

  "math-inline": (
    <span className="ts-sample__inline">
      текст <Math>a² + b²</Math> текст
    </span>
  ),
  "math-block": (
    <>
      <Line width="72%" />
      <Math>a² + b² = c²</Math>
      <Line width="58%" />
    </>
  ),
  "math-numbering": (
    <>
      <span className="ts-sample__numbered">
        <Math>E = mc²</Math>
        <span className="ts-sample__eqno">(1)</span>
      </span>
      <Line width="60%" />
    </>
  ),
  "math-frac": <Math>a⁄b</Math>,
  "math-sqrt": <Math>√(x + 1)</Math>,
  "math-sum": <Math>∑ⁿᵢ₌₁ xᵢ</Math>,
  "math-integral": <Math>∫ᵇₐ f(x) dx</Math>,
  "math-matrix": <Math>[ a b ; c d ]</Math>,
  "math-vector": <Math>( x ; y )</Math>,
  "math-cases": <Math>{"{ x = 1 ; y = 2"}</Math>,
  "math-binom": <Math>C(n, k)</Math>,
  "math-attach": <Math>x²ᵢ</Math>,
  "math-accent": <Math>x̂, ā, ṽ</Math>,
  "math-underbrace": <Math>a + b ⏟ сумма</Math>,
  "math-cancel": <Math>a̶ + b</Math>,
  "math-lr": <Math>( a/b )</Math>,
  "math-op": <Math>sin x, ker T</Math>,
  "math-bb": <Math>ℝ ℕ ℤ</Math>,
  "math-symbols": <Math>∑ α → ≤ ∞</Math>,

  figure: <Figure />,
  image: (
    <span className="ts-sample__box">
      <span className="ts-sample__box-mark" />
    </span>
  ),
  "figure-table": (
    <>
      <Cells columns={3} rows={2} />
      <span className="ts-sample__caption">Табл. 1. Подпись</span>
    </>
  ),
  table: <Cells columns={3} rows={3} />,
  "table-header": <Cells columns={3} rows={3} head />,
  grid: <Cells columns={2} rows={2} />,
  "code-block": (
    <span className="ts-sample__code">
      <span>let x = 1</span>
      <span>print(x)</span>
    </span>
  ),
  quote: (
    <span className="ts-sample__quote">
      <Line width="90%" />
      <Line width="64%" />
      <span className="ts-sample__source">— Автор</span>
    </span>
  ),
  "block-fill": <span className="ts-sample__fill" />,
  columns: (
    <span className="ts-sample__columns">
      <span>
        <Line />
        <Line width="84%" />
        <Line width="70%" />
      </span>
      <span>
        <Line />
        <Line width="76%" />
      </span>
    </span>
  ),

  circle: <span className="ts-sample__shape ts-sample__shape--circle" />,
  ellipse: <span className="ts-sample__shape ts-sample__shape--ellipse" />,
  square: <span className="ts-sample__shape ts-sample__shape--square" />,
  rect: <span className="ts-sample__shape" />,
  line: <span className="ts-sample__shape ts-sample__shape--line" />,
  polygon: <span className="ts-sample__shape ts-sample__shape--poly" />,
  gradient: <span className="ts-sample__shape ts-sample__shape--gradient" />,

  bold: (
    <span className="ts-sample__inline">
      обычный <b>жирный</b> текст
    </span>
  ),
  italic: (
    <span className="ts-sample__inline">
      обычный <i>курсив</i> текст
    </span>
  ),
  raw: (
    <span className="ts-sample__inline">
      вызов <code>image()</code> в тексте
    </span>
  ),
  strike: (
    <span className="ts-sample__inline">
      было <s>сто</s> стало
    </span>
  ),
  underline: (
    <span className="ts-sample__inline">
      слово <u>подчёркнуто</u>
    </span>
  ),
  highlight: (
    <span className="ts-sample__inline">
      слово <mark>выделено</mark>
    </span>
  ),
  smallcaps: <span className="ts-sample__smallcaps">Капитель</span>,
  super: (
    <span className="ts-sample__inline">
      x<sup>2</sup> и м<sup>3</sup>
    </span>
  ),
  sub: (
    <span className="ts-sample__inline">
      H<sub>2</sub>O
    </span>
  ),
  upper: <span className="ts-sample__upper">ВЕРХНИЙ РЕГИСТР</span>,
  linebreak: (
    <>
      <Line width="64%" />
      <Line width="48%" />
    </>
  ),
  comment: (
    <span className="ts-sample__hidden">
      на бумагу не попадёт
    </span>
  ),

  "wiki-link": (
    <span className="ts-sample__inline">
      см. <a>Методика</a>
    </span>
  ),
  "cross-ref": (
    <span className="ts-sample__inline">
      как на <a>рис. 4</a>
    </span>
  ),
  "external-link": (
    <span className="ts-sample__inline">
      сайт <a>typst.app</a>
    </span>
  ),
  cite: (
    <span className="ts-sample__inline">
      показано в <a>[1]</a>
    </span>
  ),
  "cite-supplement": (
    <span className="ts-sample__inline">
      <a>[1, с. 42]</a>
    </span>
  ),
  footnote: (
    <>
      <span className="ts-sample__inline">
        текст<sup>1</sup>
      </span>
      <span className="ts-sample__rule" />
      <span className="ts-sample__footnote">1. Примечание</span>
    </>
  ),
  bibliography: (
    <>
      <Heading size={10} text="Литература" />
      <span className="ts-sample__refs">
        <span>1. Смирнов А. — 2023</span>
        <span>2. Павлов Н. — 2019</span>
      </span>
    </>
  ),
  label: (
    <span className="ts-sample__inline">
      цель ссылки <a>&lt;sec:метод&gt;</a>
    </span>
  ),
};

/**
 * Запасной образец по разделу — для элементов, у которых своего вида нет, но
 * форма раздела о них что-то говорит (любая фигура рисуется прямоугольником,
 * любая ссылка — цветным словом). Разделы «Разметка документа» и подобные в
 * таблице отсутствуют намеренно: см. комментарий наверху файла.
 */
const FALLBACK: Partial<Record<TypstElement["category"], React.JSX.Element>> = {
  math: <Math>a² + b²</Math>,
  list: <Bullets marker={() => "•"} />,
  link: (
    <span className="ts-sample__inline">
      ссылка <a>на цель</a>
    </span>
  ),
  mark: (
    <span className="ts-sample__inline">
      обычный <b>изменённый</b> текст
    </span>
  ),
  draw: (
    <span className="ts-sample__box">
      <span className="ts-sample__box-mark" />
    </span>
  ),
};
