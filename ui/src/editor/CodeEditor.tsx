"use client";

import { useEffect, useImperativeHandle, useRef } from "react";
import { EditorState, type Extension } from "@codemirror/state";
import {
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { HighlightStyle, StreamLanguage, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import { insertSnippet, snippetFields, type InsertOptions } from "./snippet.js";

// Редактор исходника Typst — колонка «Editor pane» макета.
//
// Почему CodeMirror, а не `<textarea>`: макет требует гуттер с номерами строк,
// подсветку синтаксиса и выделение текущей строки. Первое и третье в textarea
// невозможны в принципе, второе — только подложкой из второго слоя текста,
// которая разъезжается с вводом.
//
// Подсветка своя, а не готовый пакет: языкового режима Typst для CodeMirror
// нет. Здесь ровно те категории, которые названы в handoff § Editor pane, —
// не полный разбор языка, а раскраска, и притворяться разбором она не должна.

export interface CaretPosition {
  /** Нумерация с единицы — так же, как её показывает статус-бар. */
  line: number;
  column: number;
  /** Смещение в символах: по нему вставляется разметка доски. */
  offset: number;
}

/**
 * Что редактор умеет по команде снаружи. Всё остальное он делает сам: вставка
 * — единственное действие, которое начинается не в нём (меню ⌘E), а курсор и
 * фокус нужны, чтобы это меню знало, где открыться, и куда вернуть работу.
 */
export interface CodeEditorHandle {
  /** Вставить сниппет в позицию курсора и встать в первое поле. */
  insertSnippet: (template: string, options?: InsertOptions) => void;
  /** Координаты каретки в окне — точка привязки всплывающих панелей. */
  caretCoords: () => { left: number; top: number; bottom: number } | null;
  focus: () => void;
  /**
   * Живой экземпляр редактора — для порта вставки заготовок (фича 003).
   *
   * `null`, пока редактор не смонтирован или уже размонтирован. Наружу выдаётся
   * именно экземпляр, а не готовая цель вставки: цель принадлежит фиче
   * шаблонов, и знать о ней редактору незачем.
   */
  view: () => EditorView | null;
}

export interface CodeEditorProps {
  value: string;
  onChange: (next: string) => void;
  onCaretMove?: (caret: CaretPosition) => void;
  ariaLabel: string;
  ref?: React.Ref<CodeEditorHandle>;
}

export function CodeEditor({
  value,
  onChange,
  onCaretMove,
  ariaLabel,
  ref,
}: CodeEditorProps): React.JSX.Element {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);

  useImperativeHandle(
    ref,
    () => ({
      insertSnippet: (template, options) => {
        const view = viewRef.current;
        if (view) insertSnippet(view, template, options ?? {});
      },
      caretCoords: () => {
        const view = viewRef.current;
        if (!view) return null;
        // `null` — каретка вне отрисованной части документа: у CodeMirror
        // виртуализованная прокрутка, и координат у такой позиции нет.
        const rect = view.coordsAtPos(view.state.selection.main.head);
        return rect === null
          ? null
          : { left: rect.left, top: rect.top, bottom: rect.bottom };
      },
      focus: () => viewRef.current?.focus(),
      view: () => viewRef.current,
    }),
    [],
  );

  // Колбэки живут в ref: пересоздавать редактор из-за новой ссылки на
  // обработчик нельзя — вместе с ним пропадут история правок и позиция курсора.
  const onChangeRef = useRef(onChange);
  const onCaretMoveRef = useRef(onCaretMove);
  onChangeRef.current = onChange;
  onCaretMoveRef.current = onCaretMove;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          ...baseExtensions,
          EditorView.contentAttributes.of({ "aria-label": ariaLabel }),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) onChangeRef.current(update.state.doc.toString());
            if (update.docChanged || update.selectionSet) {
              const head = update.state.selection.main.head;
              const line = update.state.doc.lineAt(head);
              onCaretMoveRef.current?.({
                line: line.number,
                column: head - line.from + 1,
                offset: head,
              });
            }
          }),
        ],
      }),
    });
    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // Пересоздание только при смене подписи, то есть при смене заметки.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ariaLabel]);

  // Внешняя замена текста — переключение заметки, вставка доски, принятие
  // внешней версии. Сравнение с текущим документом обязательно: без него
  // каждая своя же правка возвращалась бы сюда и сбрасывала курсор в начало.
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current === value) return;
    view.dispatch({
      changes: { from: 0, to: current.length, insert: value },
      // Курсор мог указывать за конец нового текста.
      selection: { anchor: Math.min(view.state.selection.main.anchor, value.length) },
    });
  }, [value]);

  return <div className="ts-editor" ref={hostRef} />;
}

/**
 * Раскраска по handoff § Editor pane.
 *
 * Цвета — ссылками на токены, а не литералами: правило проекта требует, чтобы
 * расхождение с дизайн-системой было построчным diff'ом. Исключение —
 * `#7fb3e0` для математики: этого оттенка в палитре «Спецформы» нет, он задан
 * в макете отдельным значением.
 */
const typstHighlight = HighlightStyle.define([
  { tag: tags.keyword, color: "var(--orange-400)" },
  { tag: tags.string, color: "var(--hivis-400)" },
  { tag: tags.comment, color: "var(--steel-500)", fontStyle: "italic" },
  { tag: tags.heading, color: "var(--steel-100)", fontWeight: "700" },
  { tag: tags.labelName, color: "var(--steel-500)" },
  { tag: tags.special(tags.variableName), color: "#7fb3e0" },
]);

/** Состояние построчного разбора: многострочны здесь комментарий и формула. */
interface TypstState {
  inComment: boolean;
  inMath: boolean;
}

/**
 * Ключевые слова Typst, которые встречаются с решёткой. Список намеренно
 * короткий: он покрывает то, что видно в макете и в типичной заметке, а не
 * претендует на грамматику языка.
 */
const KEYWORDS = new Set([
  "import",
  "include",
  "set",
  "show",
  "let",
  "figure",
  "image",
  "table",
  "cite",
  "ref",
  "link",
  "text",
  "heading",
  "align",
  "grid",
  "box",
  "block",
  "if",
  "else",
  "for",
  "while",
]);

const typstLanguage = StreamLanguage.define<TypstState>({
  name: "typst",
  startState: () => ({ inComment: false, inMath: false }),

  token(stream, state) {
    if (state.inComment) {
      if (stream.skipTo("*/")) {
        stream.match("*/");
        state.inComment = false;
      } else stream.skipToEnd();
      return "comment";
    }

    if (state.inMath) {
      if (stream.eat("$")) {
        state.inMath = false;
        return "special";
      }
      stream.next();
      return "special";
    }

    if (stream.eatSpace()) return null;

    // Заголовок: «=» в начале строки, как в Typst.
    if (stream.sol() && stream.match(/^=+\s/)) {
      stream.skipToEnd();
      return "heading";
    }

    if (stream.match("//")) {
      stream.skipToEnd();
      return "comment";
    }

    if (stream.match("/*")) {
      state.inComment = true;
      return "comment";
    }

    if (stream.eat('"')) {
      // Экранированная кавычка строку не закрывает.
      let escaped = false;
      for (let ch = stream.next(); ch !== undefined; ch = stream.next()) {
        if (escaped) escaped = false;
        else if (ch === "\\") escaped = true;
        else if (ch === '"') break;
      }
      return "string";
    }

    if (stream.eat("$")) {
      state.inMath = true;
      return "special";
    }

    // Метка перекрёстной ссылки: <fig:thermogram>.
    if (stream.match(/^<[^>\s]+>/)) return "labelName";

    // Ссылка на метку: @fig:thermogram.
    if (stream.match(/^@[\w:-]+/)) return "labelName";

    // Вызов с решёткой: `#import`, `#set`, `#figure` — в Typst их обозначает
    // сама решётка, поэтому проверять имя по списку незачем.
    if (stream.eat("#")) {
      stream.match(/^[a-zA-Z][\w-]*/);
      return "keyword";
    }

    // Вложенный вызов без решётки — `image(...)` внутри `#figure(...)`.
    // Здесь имя уже приходится сверять со списком: без разбора выражения
    // отличить вызов от обычного слова с открывающей скобкой нечем.
    const word = stream.match(/^[a-zA-Z][\w-]*/);
    if (Array.isArray(word)) {
      return KEYWORDS.has(word[0]) && stream.peek() === "(" ? "keyword" : null;
    }

    stream.next();
    return null;
  },

  languageData: { commentTokens: { line: "//", block: { open: "/*", close: "*/" } } },
});

/** Общая часть конфигурации — считается один раз, а не на каждый монтаж. */
const baseExtensions: Extension[] = [
  lineNumbers(),
  highlightActiveLine(),
  highlightActiveLineGutter(),
  history(),
  keymap.of([...defaultKeymap, ...historyKeymap]),
  // Поля вставленного сниппета и переходы по ним (меню ⌘E). Внутри расширения
  // приоритет выше обычной раскладки, но только пока сниппет активен.
  snippetFields,
  typstLanguage,
  syntaxHighlighting(typstHighlight),
  EditorView.lineWrapping,
];
