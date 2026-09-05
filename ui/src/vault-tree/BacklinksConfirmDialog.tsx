import type { Note } from "../wasm-core/typsidian_core.js";

/** Операция, ради которой спрашивают подтверждение (FR-027, FR-028). */
export type BacklinksOperation = "delete" | "rename";

export interface BacklinksConfirmRequest {
  operation: BacklinksOperation;
  note: Note;
  /** Куда переименовывается — только для `rename`. */
  nextPath?: string;
  /** Заметки, чьи ссылки станут битыми (`backlinks(path)` из ядра). */
  affected: Note[];
}

export interface BacklinksConfirmDialogProps {
  request: BacklinksConfirmRequest | null;
  onConfirm: () => void;
  onCancel: () => void;
}

function describeConsequence(request: BacklinksConfirmRequest): string {
  const count = request.affected.length;
  const notes = count === 1 ? "заметка" : "заметки";
  switch (request.operation) {
    case "delete":
      return `После удаления «${request.note.name}» ${count} ${notes} останутся с битой ссылкой:`;
    case "rename":
      // Содержимое ссылающихся заметок приложение не переписывает, поэтому
      // после переименования их ссылки указывают на старое имя (FR-028).
      return `После переименования «${request.note.name}» ${count} ${notes} продолжат ссылаться на старое имя, и эти ссылки станут битыми:`;
    default: {
      const exhaustive: never = request.operation;
      return exhaustive;
    }
  }
}

function confirmLabel(operation: BacklinksOperation): string {
  switch (operation) {
    case "delete":
      return "Всё равно удалить";
    case "rename":
      return "Всё равно переименовать";
    default: {
      const exhaustive: never = operation;
      return exhaustive;
    }
  }
}

/**
 * Подтверждение перед удалением или переименованием заметки, на которую есть
 * ссылки (T088, FR-027, FR-028, FR-029).
 *
 * Список приходит из `backend.backlinks(path)` — UI не вычисляет его сам
 * (contracts/vault-backend-trait.md: ссылочная проверка — ответственность
 * вызывающего кода, но «что считается ссылкой» решает ядро). Компонент
 * показывается только при непустом списке: FR-029 запрещает молчаливую
 * операцию со ссылками, но не требует подтверждения там, где ломать нечего
 * (US5 AS2).
 */
export function BacklinksConfirmDialog({
  request,
  onConfirm,
  onCancel,
}: BacklinksConfirmDialogProps): React.JSX.Element | null {
  if (!request || request.affected.length === 0) return null;

  return (
    <div
      role="alertdialog"
      aria-modal="true"
      aria-label={`Ссылки на заметку ${request.note.name}`}
    >
      <p>{describeConsequence(request)}</p>
      <ul aria-label="Заметки, чьи ссылки станут битыми">
        {request.affected.map((note) => (
          <li key={note.path}>{note.path}</li>
        ))}
      </ul>
      <div role="group" aria-label="Подтверждение операции">
        <button type="button" onClick={onConfirm}>
          {confirmLabel(request.operation)}
        </button>
        <button type="button" onClick={onCancel} autoFocus>
          Отмена
        </button>
      </div>
    </div>
  );
}
