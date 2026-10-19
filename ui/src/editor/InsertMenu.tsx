"use client";

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { Icon } from "../icons/Icons.js";
import { useI18n, type Translate } from "../i18n/index.js";
import {
  categoryName,
  ELEMENT_CATEGORIES,
  localizedElements,
  matchesQuery,
  snippetPreview,
  type ElementCategory,
  type LocalizedElement,
  type TypstElement,
} from "./typstElements.js";
import {
  MATH_SYMBOLS,
  SYMBOL_CATEGORIES,
  SYMBOL_COLUMNS,
  symbolCategoryName,
  symbolTemplate,
  type MathSymbol,
  type SymbolCategory,
} from "./mathSymbols.js";
import { parseSnippet } from "./snippet.js";
import { ElementSample } from "./elementSamples.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

// Меню вставки элементов Typst (макет «Typsidian Вставка» → § Меню вставки и
// § Палитра математики). Открывается по ⌘E у каретки.
//
// Меню ничего не вставляет само: оно объявляет наверх выбранный шаблон, а
// правит текст редактор — как палитра команд не знает, что делают её команды.
//
// Выбор один на мышь и на клавиатуру: наведение выделяет строку так же, как
// ↑↓, и предпросмотр показывает то, на что человек смотрит. Иначе подсвечены
// две строки сразу — одна под курсором, другая под стрелками, — и код в
// третьей колонке относится ко второй.
//
// Расхождение с макетом: третья колонка показывает код сниппета, а не
// отрисованный образец. Настоящая отрисовка требует компилятора (он в колонке
// предпросмотра и работает десятки миллисекунд на документ), а нарисованная от
// руки картинка утверждала бы, что элемент выглядит именно так, — по тому же
// правилу, по которому пилюля vim-режима здесь надпись, а не переключатель.

/** Положение каретки в окне: меню открывается под ней. */
export interface CaretAnchor {
  left: number;
  top: number;
  bottom: number;
}

export interface InsertRequest {
  /**
   * Запись каталога: по ней ядро решает, открывать ли форму параметров
   * (фича 008). Нет у символов математики — те вставляются сразу.
   */
  id?: string;
  template: string;
  block: boolean;
}

export interface InsertMenuProps {
  /** `null` — каретка вне видимой области: меню встаёт по центру. */
  anchor: CaretAnchor | null;
  onInsert: (request: InsertRequest) => void;
  onClose: () => void;
  /**
   * «Изображение из файла…» (фича 016, FR-004): выбор файлов и вставка к
   * каретке. Нет — пункта нет (не заметка Typst).
   */
  onInsertImageFile?: () => void;
}

type Mode = "elements" | "symbols";

/** Отступ от края окна, если меню не помещается под кареткой. */
const VIEWPORT_MARGIN = 8;
/** Зазор между кареткой и меню. */
const CARET_GAP = 6;

export function InsertMenu({
  anchor,
  onInsert,
  onClose,
  onInsertImageFile,
}: InsertMenuProps): React.JSX.Element {
  const { t } = useI18n();
  const [mode, setMode] = useState<Mode>("elements");
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<ElementCategory | null>(null);
  const [symbolCategory, setSymbolCategory] = useState<SymbolCategory>("struct");
  const [active, setActive] = useState(0);

  const menuRef = useRef<HTMLDivElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);
  // Чем выбрана строка в последний раз. Подтягивать список к выбранной строке
  // нужно только под стрелками: под мышью строка и так под курсором, а
  // доводка её «до края» уехала бы из-под пальца вместе со списком.
  const navigation = useRef<"keyboard" | "pointer">("keyboard");

  const needle = query.trim().toLowerCase();

  // Тексты элементов — на языке интерфейса: по ним и ищут, и их же вставляют.
  const catalog = useMemo(() => localizedElements(t), [t]);
  const matchedElements = useMemo(
    () => catalog.filter((element) => matchesQuery(element, needle)),
    [catalog, needle],
  );
  const elements = useMemo(
    () =>
      category === null
        ? matchedElements
        : matchedElements.filter((element) => element.category === category),
    [matchedElements, category],
  );

  // Поиск по символам идёт поперёк разделов: набрав «arrow», человек хочет
  // видеть все стрелки, а не стрелки внутри выбранного раздела.
  const symbols = useMemo(
    () =>
      needle === ""
        ? MATH_SYMBOLS.filter((symbol) => symbol.category === symbolCategory)
        : MATH_SYMBOLS.filter((symbol) => symbol.name.toLowerCase().includes(needle)),
    [needle, symbolCategory],
  );

  const rowCount = mode === "elements" ? elements.length : symbols.length;

  // Фильтр сузился — прежний индекс мог указывать за конец списка.
  useEffect(() => {
    setActive((current) => (current < rowCount ? current : 0));
  }, [rowCount]);

  const activeElement = elements[active];
  const activeSymbol = symbols[active];

  const insertElement = useCallback(
    (element: LocalizedElement) => {
      if (element.opensSymbols === true) {
        setMode("symbols");
        setQuery("");
        setActive(0);
        return;
      }
      // Открывать ли форму параметров, решает ядро по записи каталога
      // (фича 008) — меню передаёт id и уходит.
      onInsert({ id: element.id, template: element.template, block: element.block ?? false });
    },
    [onInsert],
  );

  const insertSymbol = useCallback(
    (symbol: MathSymbol) => {
      onInsert({ template: symbolTemplate(symbol), block: false });
    },
    [onInsert],
  );

  const backToElements = useCallback(() => {
    setMode("elements");
    setQuery("");
    setActive(0);
  }, []);

  /** Наведение выбирает строку: предпросмотр обязан идти за курсором. */
  const pointAt = useCallback((index: number) => {
    navigation.current = "pointer";
    setActive(index);
  }, []);

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      const step = (delta: number, wrap: boolean): void => {
        event.preventDefault();
        navigation.current = "keyboard";
        setActive((current) => {
          if (rowCount === 0) return 0;
          const next = current + delta;
          if (wrap) return (next + rowCount) % rowCount;
          return next < 0 || next >= rowCount ? current : next;
        });
      };

      // Соседний раздел на рельсе — тот же ряд, что и в списке, только слева.
      const stepCategory = (delta: number): void => {
        event.preventDefault();
        navigation.current = "keyboard";
        setCategory((current) => {
          const order: (ElementCategory | null)[] = [
            null,
            ...ELEMENT_CATEGORIES.map((section) => section.id),
          ];
          const at = order.indexOf(current);
          const next = Math.min(order.length - 1, Math.max(0, at + delta));
          return order[next] ?? null;
        });
        setActive(0);
      };

      // В списке шаг — строка и по кругу, в сетке — столбец или ряд и до края:
      // «прокрутка» сетки по кругу выглядит как случайный прыжок.
      if (event.key === "ArrowDown") step(mode === "symbols" ? SYMBOL_COLUMNS : 1, mode === "elements");
      else if (event.key === "ArrowUp") step(mode === "symbols" ? -SYMBOL_COLUMNS : -1, mode === "elements");
      else if (event.key === "ArrowRight" && mode === "symbols") step(1, false);
      else if (event.key === "ArrowLeft" && mode === "symbols") step(-1, false);
      // Пустой ввод — двигать каретку в поле поиска некуда, и ←→ достаются
      // рельсе разделов. С набранным запросом они снова правят текст: отнять у
      // поля ввода стрелки значило бы отнять у него правку.
      else if (event.key === "ArrowRight" && query === "") stepCategory(1);
      else if (event.key === "ArrowLeft" && query === "") stepCategory(-1);
      else if (event.key === "Enter") {
        event.preventDefault();
        if (mode === "elements") {
          if (activeElement) insertElement(activeElement);
        } else if (activeSymbol) insertSymbol(activeSymbol);
      } else if (event.key === "Backspace" && query === "" && mode === "symbols") {
        // Пустой ввод и Backspace — возврат из палитры символов: то же
        // движение, что и стрелка «назад» в шапке.
        event.preventDefault();
        backToElements();
      }
      // Escape намеренно не здесь: меню закрывается из любого места, и
      // слушатель на него висит в `NoteEditor` вместе с ⌘E.
    },
    [mode, rowCount, query, activeElement, activeSymbol, insertElement, insertSymbol, backToElements],
  );

  // Выбранная строка обязана оставаться видимой при навигации с клавиатуры.
  useEffect(() => {
    if (navigation.current === "pointer") return;
    const selected = listRef.current?.querySelector<HTMLElement>('[aria-selected="true"]');
    selected?.scrollIntoView({ block: "nearest" });
  }, [active, mode]);

  // Escape закрывает меню откуда угодно — общим для всех наложений
  // механизмом. Поверх меню может стоять диалог параметров; тогда Escape
  // достаётся ему, а меню закроется следующим нажатием.
  useDismissOnEscape(onClose);

  // Нажатие мимо меню закрывает его: это всплывающая панель у каретки, а не
  // модальное окно, и затемнения под ней нет.
  useEffect(() => {
    const onPointerDown = (event: PointerEvent): void => {
      const node = menuRef.current;
      if (node && event.target instanceof Node && !node.contains(event.target)) onClose();
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [onClose]);

  const position = useMenuPosition(menuRef, anchor);

  return (
    <div
      ref={menuRef}
      className="ts-insert"
      role="dialog"
      aria-label={t("insert.menu.label")}
      style={{
        left: position?.left ?? 0,
        top: position?.top ?? 0,
        // До замера меню уже отрисовано (иначе нечего мерить), но показывать
        // его в этот момент нельзя: оно прыгнуло бы на место.
        visibility: position === null ? "hidden" : "visible",
      }}
      onKeyDown={onKeyDown}
    >
      <div className="ts-insert__search">
        {mode === "elements" ? (
          <span className="ts-insert__slash" aria-hidden="true">
            /
          </span>
        ) : (
          <button
            type="button"
            className="ts-btn ts-btn--icon"
            aria-label={t("insert.menu.back")}
            onClick={backToElements}
          >
            <Icon name="chevron" size={13} className="ts-icon--flip" />
          </button>
        )}
        {/* eslint-disable-next-line jsx-a11y/no-autofocus -- меню открывается
            по ⌘E ради ввода; фокус в поле — это и есть команда */}
        <input
          autoFocus
          type="text"
          role="combobox"
          aria-expanded="true"
          aria-controls="ts-insert-list"
          aria-activedescendant={
            mode === "elements"
              ? activeElement && elementRowId(activeElement)
              : activeSymbol && symbolRowId(activeSymbol)
          }
          value={query}
          aria-label={
            mode === "elements"
              ? t("insert.menu.elementsInput")
              : t("insert.menu.symbolsInput")
          }
          placeholder={
            mode === "elements"
              ? t("insert.menu.elementsPlaceholder")
              : t("insert.menu.symbolsPlaceholder")
          }
          onChange={(event) => {
            setQuery(event.target.value);
            setActive(0);
          }}
        />
        <span className="ts-insert__shortcut">
          <Icon name="cmd" size={11} />E
        </span>
      </div>

      <div className="ts-insert__body">
        <ul className="ts-insert__rail" aria-label={t("insert.menu.sections")}>
          {mode === "elements" ? (
            <>
              <RailItem
                name={t("insert.menu.all")}
                count={matchedElements.length}
                selected={category === null}
                onSelect={() => {
                  setCategory(null);
                  setActive(0);
                }}
              />
              {ELEMENT_CATEGORIES.map((section) => (
                <RailItem
                  key={section.id}
                  name={categoryName(t, section.id)}
                  count={
                    matchedElements.filter((element) => element.category === section.id).length
                  }
                  selected={category === section.id}
                  onSelect={() => {
                    setCategory(section.id);
                    setActive(0);
                  }}
                />
              ))}
            </>
          ) : (
            SYMBOL_CATEGORIES.map((section) => (
              <RailItem
                key={section.id}
                name={symbolCategoryName(t, section.id)}
                count={MATH_SYMBOLS.filter((symbol) => symbol.category === section.id).length}
                // При поиске поперёк разделов выделять нечего: показаны все.
                selected={needle === "" && symbolCategory === section.id}
                onSelect={() => {
                  setSymbolCategory(section.id);
                  setQuery("");
                  setActive(0);
                }}
              />
            ))
          )}
        </ul>

        {mode === "elements" ? (
          <>
            <div className="ts-insert__list" ref={listRef}>
              <ul id="ts-insert-list" role="listbox" aria-label={t("insert.menu.elements")}>
                {elements.map((element, index) => (
                  <li
                    key={element.id}
                    id={elementRowId(element)}
                    role="option"
                    aria-selected={index === active}
                    className="ts-insert__row"
                    // `mousemove`, а не `mouseenter`: под стрелками список
                    // едет под неподвижным курсором, и «вход» в приехавшую
                    // строку отобрал бы выбор у клавиатуры.
                    onMouseMove={() => pointAt(index)}
                    onClick={() => insertElement(element)}
                  >
                    <Icon name={element.icon} size={13} className="ts-insert__icon" />
                    <span className="ts-insert__name">{element.name}</span>
                    <span className="ts-insert__meta">
                      {element.opensSymbols === true
                        ? t("insert.menu.symbolsCount", { count: MATH_SYMBOLS.length })
                        : snippetPreview(element.template)}
                    </span>
                  </li>
                ))}
                {elements.length === 0 && (
                  <li className="ts-insert__empty" role="presentation">
                    {t("palette.nothingFound")}
                  </li>
                )}
              </ul>
            </div>

            <div className="ts-insert__preview">
              {activeElement ? (
                <>
                  {/* Мини-«бумага» с образцом того, как элемент выглядит на
                      странице (макет § Меню вставки). Показывается не всегда:
                      у элементов, меняющих настройки документа, местного вида
                      нет, и колонка честно остаётся с одним кодом. */}
                  <ElementPaper element={activeElement} />
                  <p className="ts-insert__summary">{activeElement.summary}</p>
                  <span className="ts-eyebrow">{t("insert.menu.code")}</span>
                  <pre className="ts-insert__code">
                    <SnippetCode template={activeElement.template} />
                  </pre>
                  <span className="ts-insert__fields">
                    <Icon name="tab" size={11} />
                    {describeFields(t, activeElement)}
                  </span>
                </>
              ) : (
                <>
                  <span className="ts-eyebrow">{t("insert.menu.code")}</span>
                  <pre className="ts-insert__code" />
                </>
              )}
            </div>
          </>
        ) : (
          <div className="ts-insert__grid" ref={listRef}>
            <ul id="ts-insert-list" role="listbox" aria-label={t("insert.menu.symbols")}>
              {symbols.map((symbol, index) => (
                <li
                  key={symbol.name}
                  id={symbolRowId(symbol)}
                  role="option"
                  aria-selected={index === active}
                  aria-label={symbol.name}
                  className="ts-insert__tile"
                  onMouseMove={() => pointAt(index)}
                  onClick={() => insertSymbol(symbol)}
                >
                  <span className="ts-insert__glyph" aria-hidden="true">
                    {symbol.glyph}
                  </span>
                  <span className="ts-insert__symbol">{symbol.name}</span>
                </li>
              ))}
              {symbols.length === 0 && (
                <li className="ts-insert__empty" role="presentation">
                  {t("palette.nothingFound")}
                </li>
              )}
            </ul>
          </div>
        )}
      </div>

      {/* Нижняя полоса палитры символов (макет § Палитра математики): глиф на
          «бумаге», строка, которая уйдёт в буфер, и кнопка вставки. Полоса и
          отвечает на главный вопрос палитры — «а что именно окажется в
          тексте»: в документ идёт имя (`sum`), а не сам глиф (∑). */}
      {mode === "symbols" && (
        <div className="ts-insert__chosen">
          {activeSymbol ? (
            <>
              <span className="ts-insert__paper" aria-hidden="true">
                {activeSymbol.glyph}
              </span>
              <span className="ts-insert__chosen-code">
                {parseSnippet(symbolTemplate(activeSymbol)).text}
              </span>
              <span className="ts-spacer" />
              <button
                type="button"
                className="ts-btn ts-btn--accent"
                onClick={() => insertSymbol(activeSymbol)}
              >
                {t("insert.menu.insert")}
                <span className="ts-insert__shortcut">
                  <Icon name="cmd" size={11} />
                  <Icon name="enter" size={11} />
                </span>
              </button>
            </>
          ) : (
            <span className="ts-insert__chosen-code">{t("insert.menu.noSymbol")}</span>
          )}
        </div>
      )}

      {mode === "elements" && onInsertImageFile !== undefined && (
        <button
          type="button"
          className="ts-insert__file ts-attach-menu-item"
          onClick={() => {
            onClose();
            onInsertImageFile();
          }}
        >
          <Icon name="plus" size={12} />
          {t("attachments.insertMenu.fromFile")}
        </button>
      )}

      <div className="ts-insert__hints">
        <span>
          <Icon name="updown" size={11} />
          {t("palette.hintNavigate")}
        </span>
        <span>
          <Icon name="enter" size={11} />
          {t("insert.menu.hintInsert")}
        </span>
        <span>
          <Icon name="tab" size={11} />
          {t("insert.menu.hintFields")}
        </span>
        <span className="ts-spacer" />
        <span>
          {mode === "elements"
            ? t("insert.menu.hintSections")
            : t("insert.menu.hintSymbolName")}
        </span>
      </div>
    </div>
  );
}

/**
 * Лист «бумаги» с образцом элемента.
 *
 * Подписан «образец», а не «предпросмотр»: это схема формы, а не результат
 * сборки — компилятор сюда не зовут, иначе движение по списку упиралось бы в
 * очередь к ядру. Образца нет — нет и листа: пустая рамка утверждала бы, что
 * элемент ничего не рисует.
 */
function ElementPaper({ element }: { element: LocalizedElement }): React.JSX.Element | null {
  const { t } = useI18n();
  const sample = ElementSample({ element });
  if (sample === null) return null;
  return (
    <div className="ts-sample" aria-hidden="true">
      <div className="ts-sample__paper">{sample}</div>
      <span className="ts-sample__caption-note">{t("insert.menu.sample")}</span>
    </div>
  );
}

function RailItem({
  name,
  count,
  selected,
  onSelect,
}: {
  name: string;
  count: number;
  selected: boolean;
  onSelect: () => void;
}): React.JSX.Element {
  return (
    <li>
      <button
        type="button"
        className="ts-insert__section"
        aria-pressed={selected}
        onClick={onSelect}
      >
        <span>{name}</span>
        <span className="ts-insert__count">{count}</span>
      </button>
    </li>
  );
}

/**
 * Код сниппета с выделенными полями — то же выделение, что появится в
 * редакторе после вставки, чтобы связь «оранжевое здесь — оранжевое там» была
 * видна до нажатия.
 */
function SnippetCode({ template }: { template: string }): React.JSX.Element {
  const { text, fields } = parseSnippet(template);
  const parts: React.JSX.Element[] = [];
  let last = 0;

  fields.forEach((field, index) => {
    if (field.from > last) {
      parts.push(<span key={`text-${index}`}>{text.slice(last, field.from)}</span>);
    }
    parts.push(
      <mark key={`field-${index}`} className="ts-insert__field">
        {text.slice(field.from, field.to)}
      </mark>,
    );
    last = field.to;
  });
  if (last < text.length) parts.push(<span key="tail">{text.slice(last)}</span>);

  return <>{parts}</>;
}

/** Сколько полей заполнять после вставки. */
function describeFields(t: Translate, element: LocalizedElement): string {
  const count = parseSnippet(element.template).fields.length;
  if (count === 0) return t("insert.menu.noFields");
  return t("insert.menu.fields", { count });
}

/**
 * Меню под кареткой, но целиком в окне: снизу места может не хватить, и тогда
 * оно встаёт над строкой. Размер берётся замером, а не константой, — иначе
 * правка отступов в CSS молча ломала бы раскладку.
 */
function useMenuPosition(
  menuRef: React.RefObject<HTMLDivElement | null>,
  anchor: CaretAnchor | null,
): { left: number; top: number } | null {
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    const node = menuRef.current;
    if (!node) return;
    const box = node.getBoundingClientRect();

    const maxLeft = window.innerWidth - box.width - VIEWPORT_MARGIN;
    const wanted = anchor?.left ?? (window.innerWidth - box.width) / 2;
    const left = Math.max(VIEWPORT_MARGIN, Math.min(wanted, Math.max(VIEWPORT_MARGIN, maxLeft)));

    const below = (anchor?.bottom ?? window.innerHeight * 0.12) + CARET_GAP;
    const fits = below + box.height <= window.innerHeight - VIEWPORT_MARGIN;
    const above = (anchor?.top ?? 0) - box.height - CARET_GAP;
    const top = fits ? below : Math.max(VIEWPORT_MARGIN, above);

    setPosition({ left, top });
  }, [menuRef, anchor]);

  return position;
}

function elementRowId(element: TypstElement): string {
  return `ts-insert-${element.id}`;
}

function symbolRowId(symbol: MathSymbol): string {
  return `ts-insert-sym-${symbol.name.replace(/[^\w-]/g, "-")}`;
}
