import type { EditorView } from "@codemirror/view";
import { ChangeSet } from "@codemirror/state";
import type { TextEdit } from "../../wasm-core/typsidian_core.js";
import type { NoteDocumentRegistry } from "../../editor/noteDocuments.js";
import type { OpenNoteEditors } from "./openNoteEditors.js";

/**
 * Порт для обычного ввода (013 T047). Заметка, открытая в редакторе с
 * кареткой, правится транзакцией его CodeMirror — замена ложится в историю
 * отмены одним шагом, выделение лишь отображается через неё, документ сам
 * разносит правку соседним видам и объявляет буфер ядру. Заметка, открытая
 * только фоновой вкладкой, правится через её документ: истории отмены у неё
 * сейчас нет.
 */
export function plainEditors(
  registry: NoteDocumentRegistry | null,
  activeEditor: () => { path: string; view: EditorView } | null,
): OpenNoteEditors {
  /** Текст, отданный `textOf`, — с ним сверяется замена (обязательство 4). */
  const handedOut = new Map<string, string>();

  const current = (path: string): { text: string; view: EditorView | null } | null => {
    const active = activeEditor();
    if (active !== null && active.path === path) {
      return { text: active.view.state.doc.toString(), view: active.view };
    }
    const document = registry?.get(path);
    if (document === undefined || document.status().read !== "ready") return null;
    return { text: document.text(), view: null };
  };

  return {
    textOf: (path) => {
      const found = current(path);
      if (found === null) {
        handedOut.delete(path);
        return null;
      }
      handedOut.set(path, found.text);
      return found.text;
    },
    applyEdit: (path, edit: TextEdit) => {
      const found = current(path);
      const expected = handedOut.get(path);
      handedOut.delete(path);
      if (found === null || expected === undefined || found.text !== expected) {
        return { ok: false, error: "NoEditor" };
      }
      if (edit.from < 0 || edit.to > found.text.length || edit.from > edit.to) {
        return { ok: false, error: "NoEditor" };
      }
      if (found.view !== null) {
        // Без `selection` и `scrollIntoView`: каретка остаётся, где была.
        found.view.dispatch({
          changes: { from: edit.from, to: edit.to, insert: edit.insert },
          userEvent: "input.link",
        });
        return { ok: true };
      }
      const document = registry?.get(path);
      if (document === undefined) return { ok: false, error: "NoEditor" };
      const changes = ChangeSet.of(
        { from: edit.from, to: edit.to, insert: edit.insert },
        found.text.length,
      );
      // Вид-«источник» не прикреплён к документу: правку получат все его виды.
      document.edit({ receive: () => undefined, text: () => document.text() }, changes);
      return { ok: true };
    },
  };
}
