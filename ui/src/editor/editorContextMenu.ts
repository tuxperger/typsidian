import type { EditorView } from "@codemirror/view";
import { redo, redoDepth, undo, undoDepth } from "@codemirror/commands";
import { copyText, readText, type ContextMenuItem } from "../menu/ContextMenu.js";

/**
 * Пункты контекстного меню редактора.
 *
 * Правка текста идёт транзакциями CodeMirror — тем же путём, что ввод с
 * клавиатуры: отмена, автосохранение и `onChange` видят её как обычную правку,
 * второго пути к буферу не появляется.
 *
 * Обёртки разметки — те же символы, что пользователь набрал бы руками
 * (`*жирный*`, `_курсив_`, `` `код` ``, `$формула$`); это синтаксис Typst, а не
 * решение приложения, и каталог элементов (`typstElements.ts`) уже живёт в UI
 * по той же причине.
 */
export interface EditorMenuActions {
  /** Меню вставки элементов Typst (⌘E). */
  openInsertMenu: () => void;
  /**
   * «Параметры» элемента под курсором (фича 008, FR-006). Пункт есть всегда:
   * есть ли элемент и можно ли его править, решает ядро при вызове, а где
   * нельзя — причина называется словами (FR-011).
   */
  openParams: () => void;
  /** Подпись сочетания «Параметры элемента». */
  paramsShortcut: string;
  /** Выбор доски для вставки. */
  chooseBoard: () => void;
  /** Сохранить заметку как заготовку; `undefined` — возможность не подключена. */
  saveAsTemplate?: (() => void) | undefined;
  /** Итог действия без своего места на экране. */
  notify: (message: string) => void;
  /** Подписи сочетаний — те же, что в палитре. */
  insertShortcut: string;
}

const MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
const key = (letter: string, shift = false) =>
  MAC ? `${shift ? "⇧" : ""}⌘${letter}` : `Ctrl+${shift ? "Shift+" : ""}${letter}`;

/**
 * Правый клик вне выделения ставит курсор туда, где кликнули, — как в любом
 * редакторе: иначе «Вставить» уходило бы в прежнее место, которое пользователь
 * уже не видит. Клик внутри выделения его сохраняет — чтобы было что копировать.
 */
export function placeCaretAt(view: EditorView, x: number, y: number): void {
  const pos = view.posAtCoords({ x, y });
  const { from, to } = view.state.selection.main;
  if (pos !== null && (pos < from || pos > to)) {
    view.dispatch({ selection: { anchor: pos } });
  }
}

export function editorMenuItems(
  view: EditorView,
  actions: EditorMenuActions,
): ContextMenuItem[] {
  const { from, to, empty } = view.state.selection.main;
  const selected = view.state.sliceDoc(from, to);

  const wrap = (open: string, close: string = open) => () => {
    const range = view.state.selection.main;
    const text = view.state.sliceDoc(range.from, range.to);
    view.dispatch({
      changes: { from: range.from, to: range.to, insert: `${open}${text}${close}` },
      // Выделение остаётся на том же тексте, уже внутри разметки: так обёртку
      // можно тут же снять отменой или добавить вторую.
      selection: {
        anchor: range.from + open.length,
        head: range.from + open.length + text.length,
      },
      userEvent: "input",
    });
    view.focus();
  };

  return [
    {
      label: "Отменить",
      shortcut: key("Z"),
      disabled: undoDepth(view.state) === 0,
      run: () => {
        undo(view);
        view.focus();
      },
    },
    {
      label: "Повторить",
      shortcut: key("Z", true),
      disabled: redoDepth(view.state) === 0,
      run: () => {
        redo(view);
        view.focus();
      },
    },
    { kind: "separator" },
    {
      label: "Вырезать",
      shortcut: key("X"),
      disabled: empty,
      run: async () => {
        const failure = await copyText(selected);
        if (failure) {
          actions.notify(failure);
          return;
        }
        view.dispatch({
          changes: { from, to, insert: "" },
          userEvent: "delete.cut",
        });
        view.focus();
      },
    },
    {
      label: "Копировать",
      shortcut: key("C"),
      disabled: empty,
      run: async () => {
        const failure = await copyText(selected);
        if (failure) actions.notify(failure);
        view.focus();
      },
    },
    {
      label: "Вставить",
      shortcut: key("V"),
      run: async () => {
        const result = await readText();
        if ("error" in result) {
          actions.notify(result.error);
          view.focus();
          return;
        }
        view.dispatch(view.state.replaceSelection(result.text), {
          userEvent: "input.paste",
          scrollIntoView: true,
        });
        view.focus();
      },
    },
    {
      label: "Выделить всё",
      shortcut: key("A"),
      run: () => {
        view.dispatch({ selection: { anchor: 0, head: view.state.doc.length } });
        view.focus();
      },
    },
    { kind: "separator" },
    { label: "Жирный", disabled: empty, run: wrap("*") },
    { label: "Курсив", disabled: empty, run: wrap("_") },
    { label: "Моноширинный", disabled: empty, run: wrap("`") },
    { label: "Формула", disabled: empty, run: wrap("$") },
    { kind: "separator" },
    {
      label: "Вставить элемент Typst…",
      shortcut: actions.insertShortcut,
      run: actions.openInsertMenu,
    },
    {
      label: "Параметры элемента…",
      shortcut: actions.paramsShortcut,
      run: actions.openParams,
    },
    { label: "Вставить доску…", run: actions.chooseBoard },
    ...(actions.saveAsTemplate
      ? [
          { kind: "separator" as const },
          { label: "Сохранить как заготовку…", run: actions.saveAsTemplate },
        ]
      : []),
  ];
}
