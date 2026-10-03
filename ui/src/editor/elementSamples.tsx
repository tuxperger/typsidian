"use client";

import { useI18n, type Translate } from "../i18n/index.js";
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
  const { t } = useI18n();
  const byId = samples(t)[element.id];
  if (byId) return byId;
  return fallback(t)[element.category] ?? null;
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
function Figure({ caption }: { caption: string }) {
  return (
    <>
      <span className="ts-sample__box">
        <span className="ts-sample__box-mark" />
      </span>
      <span className="ts-sample__caption">{caption}</span>
    </>
  );
}

/**
 * Строка с выделенным словом. `%` в тексте каталога — место слова: порядок слов
 * вокруг него решает язык, а не разметка (фича 010).
 */
function Around({ text, children }: { text: string; children: React.ReactNode }) {
  const [before = "", after = ""] = text.split("%");
  return (
    <span className="ts-sample__inline">
      {before}
      {children}
      {after}
    </span>
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
function samples(t: Translate): Record<string, React.JSX.Element> {
  return {
    "heading-1": (
      <>
        <Heading size={13} text={t("insert.sample.section")} />
        <Para />
      </>
    ),
    "heading-2": (
      <>
        <Heading size={11} text={t("insert.sample.subsection")} />
        <Para />
      </>
    ),
    "heading-3": (
      <>
        <Heading size={10} text={t("insert.sample.item")} />
        <Para />
      </>
    ),
    "heading-4": (
      <>
        <Heading size={9} text={t("insert.sample.subitem")} />
        <Para />
      </>
    ),
    title: (
      <>
        <span className="ts-sample__title">{t("insert.sample.workTitle")}</span>
        <span className="ts-sample__byline">{t("insert.sample.byline")}</span>
        <Para />
      </>
    ),
    outline: (
      <>
        <Heading size={10} text={t("insert.sample.contents")} />
        <span className="ts-sample__toc">
          <span>
            {t("insert.sample.introduction")} <i /> 3
          </span>
          <span>
            {t("insert.sample.method")} <i /> 7
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
        <span className="ts-sample__term">{t("insert.sample.term")}</span>
        <Line width="84%" />
        <span className="ts-sample__term">{t("insert.sample.second")}</span>
        <Line width="66%" />
      </span>
    ),
    "enum-numbering": <Bullets marker={(index) => `${String.fromCharCode(97 + index)})`} />,
    "list-marker": <Bullets marker={() => "—"} />,
  
    "math-inline": (
      <Around text={t("insert.sample.textAround")}>
        <Math>a² + b²</Math>
      </Around>
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
    "math-underbrace": <Math>a + b ⏟ {t("insert.sample.sum")}</Math>,
    "math-cancel": <Math>a̶ + b</Math>,
    "math-lr": <Math>( a/b )</Math>,
    "math-op": <Math>sin x, ker T</Math>,
    "math-bb": <Math>ℝ ℕ ℤ</Math>,
    "math-symbols": <Math>∑ α → ≤ ∞</Math>,
  
    figure: <Figure caption={t("insert.sample.figureCaption")} />,
    image: (
      <span className="ts-sample__box">
        <span className="ts-sample__box-mark" />
      </span>
    ),
    "figure-table": (
      <>
        <Cells columns={3} rows={2} />
        <span className="ts-sample__caption">{t("insert.sample.tableCaption")}</span>
      </>
    ),
    table: <Cells columns={3} rows={3} />,
    "table-header": <Cells columns={3} rows={3} head />,
    grid: <Cells columns={2} rows={2} />,
    "code-block": (
      <span className="ts-sample__code">
        <span>let x = 1</span>{/* i18n-ignore: образец кода */}
        <span>print(x)</span>{/* i18n-ignore: образец кода */}
      </span>
    ),
    quote: (
      <span className="ts-sample__quote">
        <Line width="90%" />
        <Line width="64%" />
        <span className="ts-sample__source">{t("insert.sample.quoteSource")}</span>
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
      <Around text={t("insert.sample.regularAround")}>
        <b>{t("insert.sample.boldWord")}</b>
      </Around>
    ),
    italic: (
      <Around text={t("insert.sample.regularAround")}>
        <i>{t("insert.sample.italicWord")}</i>
      </Around>
    ),
    raw: (
      <Around text={t("insert.sample.callAround")}>
        <code>image()</code>
      </Around>
    ),
    strike: (
      <Around text={t("insert.sample.strikeAround")}>
        <s>{t("insert.sample.strikeWord")}</s>
      </Around>
    ),
    underline: (
      <Around text={t("insert.sample.wordAround")}>
        <u>{t("insert.sample.underlinedWord")}</u>
      </Around>
    ),
    highlight: (
      <Around text={t("insert.sample.wordAround")}>
        <mark>{t("insert.sample.highlightedWord")}</mark>
      </Around>
    ),
    smallcaps: <span className="ts-sample__smallcaps">{t("insert.sample.smallcaps")}</span>,
    super: (
      <span className="ts-sample__inline">
        x<sup>2</sup> {t("insert.sample.and")} {t("insert.sample.metre")}<sup>3</sup>
      </span>
    ),
    sub: (
      <span className="ts-sample__inline">
        H<sub>2</sub>O
      </span>
    ),
    upper: <span className="ts-sample__upper">{t("insert.sample.upper")}</span>,
    linebreak: (
      <>
        <Line width="64%" />
        <Line width="48%" />
      </>
    ),
    comment: (
      <span className="ts-sample__hidden">
        {t("insert.sample.comment")}
      </span>
    ),
  
    "wiki-link": (
      <Around text={t("insert.sample.seeAround")}>
        <a>{t("insert.sample.method")}</a>
      </Around>
    ),
    "cross-ref": (
      <Around text={t("insert.sample.asInAround")}>
        <a>{t("insert.sample.figRef")}</a>
      </Around>
    ),
    "external-link": (
      <Around text={t("insert.sample.siteAround")}>
        <a>typst.app</a>{/* i18n-ignore: адрес сайта */}
      </Around>
    ),
    cite: (
      <Around text={t("insert.sample.shownInAround")}>
        <a>[1]</a>
      </Around>
    ),
    "cite-supplement": (
      <span className="ts-sample__inline">
        <a>{t("insert.sample.citeSupplement")}</a>
      </span>
    ),
    footnote: (
      <>
        <span className="ts-sample__inline">
          {t("insert.sample.text")}<sup>1</sup>
        </span>
        <span className="ts-sample__rule" />
        <span className="ts-sample__footnote">{t("insert.sample.footnote")}</span>
      </>
    ),
    bibliography: (
      <>
        <Heading size={10} text={t("insert.sample.bibliography")} />
        <span className="ts-sample__refs">
          <span>{t("insert.sample.ref1")}</span>
          <span>{t("insert.sample.ref2")}</span>
        </span>
      </>
    ),
    label: (
      <Around text={t("insert.sample.labelAround")}>
        <a>&lt;{t("insert.sample.labelName")}&gt;</a>
      </Around>
    ),
  };
}

/**
 * Запасной образец по разделу — для элементов, у которых своего вида нет, но
 * форма раздела о них что-то говорит (любая фигура рисуется прямоугольником,
 * любая ссылка — цветным словом). Разделы «Разметка документа» и подобные в
 * таблице отсутствуют намеренно: см. комментарий наверху файла.
 */
function fallback(t: Translate): Partial<Record<TypstElement["category"], React.JSX.Element>> {
  return {
    math: <Math>a² + b²</Math>,
    list: <Bullets marker={() => "•"} />,
    link: (
      <Around text={t("insert.sample.linkAround")}>
        <a>{t("insert.sample.linkTarget")}</a>
      </Around>
    ),
    mark: (
      <Around text={t("insert.sample.regularAround")}>
        <b>{t("insert.sample.changedWord")}</b>
      </Around>
    ),
    draw: (
      <span className="ts-sample__box">
        <span className="ts-sample__box-mark" />
      </span>
    ),
  };
}
