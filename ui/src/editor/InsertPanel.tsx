"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Icon } from "../icons/Icons.js";
import { elementById, type TypstElement } from "./typstElements.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

// Панель вставки — полоса 34px между тулбаром колонки и кодом (макет
// «Typsidian Вставка» → § Панель вставки).
//
// Она **для мыши и не заменяет меню**: то же самое делается ⌘E и «/», а панель
// — короткий путь к дюжине самых частых элементов. Поэтому её можно убрать
// (кнопка «Панель» в тулбаре заметки), и поэтому её крайняя правая кнопка ведёт
// в полный каталог, а не в ещё один список.
//
// Ничего своего панель не вставляет: она объявляет наверх выбранный элемент
// каталога, а текст правит редактор — тем же путём, что и меню. Второго набора
// шаблонов здесь нет и быть не должно.

/**
 * Состав панели — именами элементов каталога, в порядке макета: заголовки,
 * два списка, формула, рисунок, таблица, код, цитата, вики-ссылка, метка,
 * жирный.
 *
 * Заголовки стоят отдельно: их три уровня, и в полосе они прячутся за один
 * выпадающий список — иначе три кнопки из двенадцати уходят под одно и то же.
 */
const HEADINGS = ["heading-1", "heading-2", "heading-3"];
const BUTTONS = [
  "list-bullet",
  "list-numbered",
  "math-inline",
  "figure",
  "table",
  "code-block",
  "quote",
  "wiki-link",
  "label",
  "bold",
];

/** Подпись уровня заголовка в выпадающем списке: `=`, `==`, `===`. */
const HEADING_MARKS: Record<string, string> = {
  "heading-1": "=",
  "heading-2": "==",
  "heading-3": "===",
};

export interface InsertPanelProps {
  /** Подпись действующего сочетания меню — «⌘E», пока его не переназначили. */
  shortcut: string;
  onInsert: (element: TypstElement) => void;
  onOpenMenu: () => void;
  /**
   * «Параметры» элемента под кареткой (фича 008, FR-006). `null` — каретка не
   * в элементе с параметрами: кнопки нет. Недоступная правка показывается
   * неактивной кнопкой с причиной (FR-011).
   */
  params?: { available: boolean; reason?: string } | null;
  paramsShortcut?: string;
  onParams?: () => void;
}

export function InsertPanel({
  shortcut,
  onInsert,
  onOpenMenu,
  params = null,
  paramsShortcut = "",
  onParams,
}: InsertPanelProps): React.JSX.Element {
  const [headingsOpen, setHeadingsOpen] = useState(false);
  const headingsRef = useRef<HTMLDivElement | null>(null);

  const closeHeadings = useCallback(() => setHeadingsOpen(false), []);
  useDismissOnEscape(headingsOpen ? closeHeadings : null);

  // Нажатие мимо закрывает список уровней: это всплывающая панелька у кнопки,
  // а не модальное окно.
  useEffect(() => {
    if (!headingsOpen) return undefined;
    const onPointerDown = (event: PointerEvent): void => {
      const node = headingsRef.current;
      if (node && event.target instanceof Node && !node.contains(event.target)) {
        closeHeadings();
      }
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [headingsOpen, closeHeadings]);

  const insert = (element: TypstElement | undefined): void => {
    if (element) onInsert(element);
  };

  return (
    <div className="ts-insertbar" role="toolbar" aria-label="Панель вставки">
      {/* Прокручивается только ряд элементов. Кнопка полного каталога стоит
          вне его: в узкой колонке она уехала бы за край первой, а это
          единственный выход к остальным ста элементам. */}
      <div className="ts-insertbar__scroll">
      <div className="ts-insertbar__group" ref={headingsRef}>
        <button
          type="button"
          className="ts-btn"
          aria-haspopup="menu"
          aria-expanded={headingsOpen}
          title="Заголовок: уровень 1–3"
          onClick={() => setHeadingsOpen((open) => !open)}
        >
          <Icon name="heading" size={12} />
          Заголовок
          <Icon name="chevron" size={10} className="ts-icon--down" />
        </button>

        {headingsOpen && (
          <ul className="ts-insertbar__menu" role="menu" aria-label="Уровень заголовка">
            {HEADINGS.map((id) => {
              const element = elementById(id);
              if (!element) return null;
              return (
                <li key={id} role="none">
                  <button
                    type="button"
                    role="menuitem"
                    className="ts-insertbar__level"
                    onClick={() => {
                      closeHeadings();
                      insert(element);
                    }}
                  >
                    <span className="ts-insertbar__mark">{HEADING_MARKS[id]}</span>
                    {element.name}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      <span className="ts-insertbar__sep" />

      {BUTTONS.map((id) => {
        const element = elementById(id);
        if (!element) return null;
        return (
          <button
            key={id}
            type="button"
            className="ts-btn ts-btn--icon"
            title={element.name}
            aria-label={element.name}
            onClick={() => insert(element)}
          >
            <Icon name={element.icon} size={13} />
          </button>
        );
      })}

      </div>

      {/* Правка готового блока. Стоит вне прокрутки и рядом с каталогом: это
          не «ещё один элемент», а действие над тем, что уже в тексте. */}
      {params !== null && onParams && (
        <button
          type="button"
          className="ts-btn ts-btn--on"
          disabled={!params.available}
          title={
            params.available
              ? `Параметры элемента под кареткой${paramsShortcut ? ` (${paramsShortcut})` : ""}`
              : params.reason
          }
          onClick={onParams}
        >
          <Icon name="terminal" size={12} />
          Параметры
        </button>
      )}

      {/* Правый край полосы ведёт в полный каталог — панель не притворяется
          заменой меню, а объявляет, где остальное. */}
      <button type="button" className="ts-btn" onClick={onOpenMenu}>
        Все элементы
        {shortcut !== "" && <span className="ts-insertbar__shortcut">{shortcut}</span>}
      </button>
    </div>
  );
}

const STORAGE_KEY = "typsidian.insertPanel";

/**
 * Показана ли панель.
 *
 * Хранится в `localStorage`, как и размеры колонок: это состояние окна, а не
 * настройка папки заметок. Место для неё в настройках (как в handoff) означало
 * бы новое поле в `VaultOverrides`, то есть правку ядра и пересборку wasm, —
 * до тех пор выключатель живёт там же, где остальные предпочтения раскладки.
 *
 * Первый кадр всегда «показана», даже если в хранилище лежит другое: `Workspace`
 * рендерится и на сервере (apps/web — Next.js), где хранилища нет, и прочитанное
 * здесь разошлось бы с серверной разметкой.
 */
export function useInsertPanelVisible(): [boolean, () => void] {
  const [visible, setVisible] = useState(true);

  useEffect(() => {
    try {
      if (window.localStorage.getItem(STORAGE_KEY) === "hidden") setVisible(false);
    } catch {
      // Хранилища нет или оно закрыто — панель просто останется показанной.
    }
  }, []);

  const toggle = useCallback(() => {
    setVisible((current) => {
      const next = !current;
      try {
        window.localStorage.setItem(STORAGE_KEY, next ? "shown" : "hidden");
      } catch {
        // Не записалось — панель всё равно переключилась, потеряется только
        // память о выборе.
      }
      return next;
    });
  }, []);

  return [visible, toggle];
}
