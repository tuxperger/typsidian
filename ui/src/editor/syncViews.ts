import { Annotation, Transaction, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import type { DocumentView, NoteDocument } from "./noteDocument.js";

// Правки между видами одного документа (фича 014, research.md §3).
//
// Правка в одной группе уходит в документ (`NoteDocument.edit`), а оттуда —
// остальным видам того же пути транзакцией с тем же `ChangeSet`. Транзакция
// помечена аннотацией «от соседа» и не входит в историю отмены: CodeMirror
// перебазирует свою историю поверх чужих правок, как при совместном
// редактировании, и `Ctrl+Z` в группе отменяет набранное в этой группе.
//
// Расширение общее для всех видов и не держит замыканий: состояние вида
// (`EditorState`) переживает вкладку в `tabEditorStates` и потом поднимается в
// новом экземпляре редактора, а связь с документом лежит снаружи состояния.

/** Транзакция пришла от соседнего вида или заменила весь текст — документу её не возвращаем. */
export const fromNeighbor = Annotation.define<boolean>();

interface Link {
  document: NoteDocument;
  handle: DocumentView;
}

/** Связь каждого живого вида с документом. Вне `EditorState`, чтобы тот оставался переносимым. */
const links = new WeakMap<EditorView, Link>();

/** Отдаёт правки пользователя документу. Одно общее расширение на все виды. */
export const documentSync: Extension = EditorView.updateListener.of((update) => {
  const link = links.get(update.view);
  if (link === undefined) return;
  for (const transaction of update.transactions) {
    if (transaction.docChanged && transaction.annotation(fromNeighbor) !== true) {
      link.document.edit(link.handle, transaction.changes);
    }
  }
});

/**
 * Подключает вид к документу: с этого момента его правки уходят в документ, а
 * правки соседей приходят в него. Возвращает отключение.
 */
export function connectView(view: EditorView, document: NoteDocument): () => void {
  const handle: DocumentView = {
    receive(changes, whole) {
      const head = view.state.selection.main.head;
      view.dispatch({
        changes,
        annotations: [fromNeighbor.of(true), Transaction.addToHistory.of(false)],
        // Замена всего текста: каретка не должна остаться за концом нового.
        ...(whole ? { selection: { anchor: Math.min(head, changes.newLength) } } : {}),
      });
    },
    text: () => view.state.doc.toString(),
  };
  links.set(view, { document, handle });
  const detach = document.attach(handle);
  return () => {
    detach();
    links.delete(view);
  };
}
