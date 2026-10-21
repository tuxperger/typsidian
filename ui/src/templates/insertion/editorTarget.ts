import { EditorSelection } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import type { InsertionEdit, InsertionPoint } from "../../wasm-core/typsidian_core.js";
import type { InsertionTarget, InsertResult } from "./index.js";

// Реализация порта вставки поверх редактора заметки (T038-T040).
//
// # Почему одна транзакция, а не «прочитать, склеить, записать»
//
// FR-033 требует, чтобы одна отмена возвращала текст целиком, а предыдущие
// правки пользователя при этом остались откатываемыми. Реализация «прочитать
// значение, склеить строку, записать обратно» заменяет документ целиком: она
// даёт одну запись в истории, но эта запись перекрывает весь текст, и отмена
// после неё стирает границы предыдущих правок вместо того, чтобы их сохранить.
// Один `dispatch` с точечным `changes` даёт то же одно нажатие Ctrl+Z и
// оставляет историю нетронутой (research.md §6).
//
// Это самое рискованное место фичи: ошибка здесь не ломает сборку и не видна в
// тестах ядра — она проявляется тем, что пользователь теряет возможность
// откатить свои предыдущие правки. Поэтому шаг 4 Сценария 5 в `quickstart.md`
// проверяет именно **вторую** отмену, а не первую.

/**
 * Цель вставки поверх CodeMirror.
 *
 * `view` — функция, а не значение: редактор пересоздаётся при смене заметки, и
 * цель, захватившая старый экземпляр, вставляла бы в размонтированный документ.
 */
export function editorTarget(
  view: () => EditorView | null,
): InsertionTarget {
  return {
    insertionPoint(): InsertionPoint | null {
      const editor = view();
      if (!editor) return null;
      const range = editor.state.selection.main;
      return {
        offset: range.head,
        // Пустое выделение выделением не является: FR-035 говорит про замену
        // выделенного фрагмента, а у курсора без выделения заменять нечего.
        selection:
          range.from === range.to
            ? undefined
            : { start: range.from, end: range.to },
      };
    },

    textLength(): number {
      return view()?.state.doc.length ?? 0;
    },

    apply(edit: InsertionEdit): InsertResult {
      const editor = view();
      if (!editor) return { ok: false, error: { kind: "NoEditor" } };

      // Одна транзакция — одна единица истории отмены. `userEvent` объявлен
      // вставкой: `history()` склеивает соседние правки по типу события, и
      // одиночная `input.paste` ни с чем не склеивается — ни с набором до неё,
      // ни с набором после.
      editor.dispatch({
        changes: {
          from: edit.replace.start,
          to: edit.replace.end,
          insert: edit.text,
        },
        selection: EditorSelection.cursor(edit.caret),
        scrollIntoView: true,
        userEvent: "input.paste",
      });
      // FR-034: работа продолжается в тексте, а не в закрытой галерее.
      editor.focus();
      return { ok: true };
    },
  };
}
