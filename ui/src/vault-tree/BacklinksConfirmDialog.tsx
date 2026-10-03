import { useI18n, type Translate } from "../i18n/index.js";
import type { Note } from "../wasm-core/typsidian_core.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

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

function describeConsequence(t: Translate, request: BacklinksConfirmRequest): string {
  const params = { name: request.note.name, count: request.affected.length };
  switch (request.operation) {
    case "delete":
      return t("tree.backlinks.deleteConsequence", params);
    case "rename":
      // Содержимое ссылающихся заметок приложение не переписывает, поэтому
      // после переименования их ссылки указывают на старое имя (FR-028).
      return t("tree.backlinks.renameConsequence", params);
    default: {
      const exhaustive: never = request.operation;
      return exhaustive;
    }
  }
}

function confirmLabel(t: Translate, operation: BacklinksOperation): string {
  switch (operation) {
    case "delete":
      return t("tree.backlinks.deleteAnyway");
    case "rename":
      return t("tree.backlinks.renameAnyway");
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
  const { t } = useI18n();
  const shown = request !== null && request.affected.length > 0;
  // Escape здесь — то же, что «Отмена»: операция ещё не началась, и отказ от
  // неё ничего не теряет.
  useDismissOnEscape(shown ? onCancel : null);

  if (!shown) return null;

  return (
    <div
      className="ts-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-label={t("tree.backlinks.dialogLabel", { name: request.note.name })}
    >
      <p>{describeConsequence(t, request)}</p>
      <ul aria-label={t("tree.backlinks.affectedLabel")}>
        {request.affected.map((note) => (
          <li key={note.path}>{note.path}</li>
        ))}
      </ul>
      <div role="group" aria-label={t("tree.backlinks.confirmGroup")}>
        <button type="button" onClick={onConfirm}>
          {confirmLabel(t, request.operation)}
        </button>
        <button type="button" onClick={onCancel} autoFocus>
          {t("common.cancel")}
        </button>
      </div>
    </div>
  );
}
