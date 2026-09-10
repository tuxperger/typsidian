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
import { plural } from "../plural.js";
import {
  ELEMENT_CATEGORIES,
  matchesQuery,
  snippetPreview,
  TYPST_ELEMENTS,
  type ElementCategory,
  type TypstElement,
} from "./typstElements.js";
import {
  MATH_SYMBOLS,
  SYMBOL_CATEGORIES,
  SYMBOL_COLUMNS,
  symbolTemplate,
  type MathSymbol,
  type SymbolCategory,
} from "./mathSymbols.js";
import { parseSnippet } from "./snippet.js";

// Меню вставки элементов Typst (макет «Typsidian Вставка» → § Меню вставки и
// § Палитра математики). Открывается по ⌘E у каретки.
//
// Меню ничего не вставляет само: оно объявляет наверх выбранный шаблон, а
// правит текст редактор — как палитра команд не знает, что делают её команды.
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
  template: string;
  block: boolean;
}

export interface InsertMenuProps {
  /** `null` — каретка вне видимой области: меню встаёт по центру. */
  anchor: CaretAnchor | null;
  onInsert: (request: InsertRequest) => void;
  onClose: () => void;
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
}: InsertMenuProps): React.JSX.Element {
  const [mode, setMode] = useState<Mode>("elements");
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<ElementCategory | null>(null);
  const [symbolCategory, setSymbolCategory] = useState<SymbolCategory>("struct");
  const [active, setActive] = useState(0);

  const menuRef = useRef<HTMLDivElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);

  const needle = query.trim().toLowerCase();

  const matchedElements = useMemo(
    () => TYPST_ELEMENTS.filter((element) => matchesQuery(element, needle)),
    [needle],
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
    (element: TypstElement) => {
      if (element.opensSymbols === true) {
        setMode("symbols");
        setQuery("");
        setActive(0);
        return;
      }
      onInsert({ template: element.template, block: element.block ?? false });
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

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      const step = (delta: number, wrap: boolean): void => {
        event.preventDefault();
        setActive((current) => {
          if (rowCount === 0) return 0;
          const next = current + delta;
          if (wrap) return (next + rowCount) % rowCount;
          return next < 0 || next >= rowCount ? current : next;
        });
      };

      // В списке шаг — строка и по кругу, в сетке — столбец или ряд и до края:
      // «прокрутка» сетки по кругу выглядит как случайный прыжок.
      if (event.key === "ArrowDown") step(mode === "symbols" ? SYMBOL_COLUMNS : 1, mode === "elements");
      else if (event.key === "ArrowUp") step(mode === "symbols" ? -SYMBOL_COLUMNS : -1, mode === "elements");
      else if (event.key === "ArrowRight" && mode === "symbols") step(1, false);
      else if (event.key === "ArrowLeft" && mode === "symbols") step(-1, false);
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
    const selected = listRef.current?.querySelector<HTMLElement>('[aria-selected="true"]');
    selected?.scrollIntoView({ block: "nearest" });
  }, [active, mode]);

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
      aria-label="Вставка элемента Typst"
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
            aria-label="Назад к списку элементов"
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
              ? "Элемент Typst: заголовок, список, формула"
              : "Имя символа: sum, alpha, arrow"
          }
          placeholder={
            mode === "elements"
              ? "элемент Typst: заголовок, список, формула…"
              : "имя символа: sum, alpha, arrow…"
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
        <ul className="ts-insert__rail" aria-label="Разделы">
          {mode === "elements" ? (
            <>
              <RailItem
                name="Все"
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
                  name={section.name}
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
                name={section.name}
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
              <ul id="ts-insert-list" role="listbox" aria-label="Элементы Typst">
                {elements.map((element, index) => (
                  <li
                    key={element.id}
                    id={elementRowId(element)}
                    role="option"
                    aria-selected={index === active}
                    className="ts-insert__row"
                    onClick={() => insertElement(element)}
                  >
                    <Icon name={element.icon} size={13} className="ts-insert__icon" />
                    <span className="ts-insert__name">{element.name}</span>
                    <span className="ts-insert__meta">
                      {element.opensSymbols === true
                        ? plural(MATH_SYMBOLS.length, "символ", "символа", "символов")
                        : snippetPreview(element.template)}
                    </span>
                  </li>
                ))}
                {elements.length === 0 && (
                  <li className="ts-insert__empty" role="presentation">
                    ничего не найдено
                  </li>
                )}
              </ul>
            </div>

            <div className="ts-insert__preview">
              <span className="ts-eyebrow">код</span>
              {activeElement ? (
                <>
                  <pre className="ts-insert__code">
                    <SnippetCode template={activeElement.template} />
                  </pre>
                  <span className="ts-insert__fields">
                    <Icon name="tab" size={11} />
                    {describeFields(activeElement)}
                  </span>
                </>
              ) : (
                <pre className="ts-insert__code" />
              )}
            </div>
          </>
        ) : (
          <div className="ts-insert__grid" ref={listRef}>
            <ul id="ts-insert-list" role="listbox" aria-label="Символы Typst">
              {symbols.map((symbol, index) => (
                <li
                  key={symbol.name}
                  id={symbolRowId(symbol)}
                  role="option"
                  aria-selected={index === active}
                  aria-label={symbol.name}
                  className="ts-insert__tile"
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
                  ничего не найдено
                </li>
              )}
            </ul>
          </div>
        )}
      </div>

      <div className="ts-insert__hints">
        <span>
          <Icon name="updown" size={11} />
          навигация
        </span>
        <span>
          <Icon name="enter" size={11} />
          вставить
        </span>
        <span>
          <Icon name="tab" size={11} />
          поля сниппета
        </span>
        <span className="ts-spacer" />
        <span>
          {mode === "elements"
            ? "esc закрыть"
            : "в текст идёт имя символа, не глиф"}
        </span>
      </div>
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
function describeFields(element: TypstElement): string {
  const count = parseSnippet(element.template).fields.length;
  if (count === 0) return "без полей";
  return `${plural(count, "поле", "поля", "полей")} по ⇥`;
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
