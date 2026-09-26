"use client";

import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

// Выбор доски для вставки в открытую заметку (T045, T046, FR-018, SC-007).
//
// SC-007 даёт не более трёх действий от открытой заметки до вставленной доски:
// «Вставить доску» → выбор доски в списке. Второе действие и есть вставка —
// отдельного подтверждения нет намеренно, оно было бы третьим действием без
// новой информации для пользователя.

/** Доска в списке выбора. Форма совпадает с записью дерева vault. */
export interface BoardChoice {
  path: string;
  name: string;
}

export interface InsertBoardDialogProps {
  /** `false` — диалога нет. */
  isOpen: boolean;
  /** Доски vault. Пустой список — штатный случай, а не ошибка (US2 AS2). */
  boards: BoardChoice[];
  /** Идёт построение снимка и запись файла. */
  isInserting?: boolean;
  /** Причина отказа словами; `null` — отказа не было. */
  error?: string | null;
  onInsert: (boardPath: string) => void;
  /** Создать доску, когда вставлять пока нечего (US2 AS2). */
  onCreateBoard?: () => void;
  onCancel: () => void;
}

export function InsertBoardDialog({
  isOpen,
  boards,
  isInserting = false,
  error = null,
  onInsert,
  onCreateBoard,
  onCancel,
}: InsertBoardDialogProps): React.JSX.Element | null {
  // Пока идёт запись снимка, Escape не отменяет: файл уже пишется, и закрытие
  // диалога не отменило бы начатое, а только спрятало бы его исход.
  useDismissOnEscape(isOpen && !isInserting ? onCancel : null);

  if (!isOpen) return null;

  return (
    <div
      className="ts-dialog ts-dialog--choice"
      role="dialog"
      aria-modal="true"
      aria-label="Вставить доску в заметку"
    >
      {error !== null && <p role="alert">{error}</p>}

      {boards.length === 0 ? (
        // US2 AS2: в vault ещё нет ни одной доски. Пустой список — это не
        // отказ, поэтому здесь не `role="alert"`, а объяснение и выход из
        // положения одним действием.
        <>
          <p>
            В этом vault ещё нет досок. Создайте доску — после этого её можно
            будет вставить в заметку.
          </p>
          <div role="group" aria-label="Что сделать">
            {onCreateBoard && (
              <button type="button" onClick={onCreateBoard}>
                Создать доску
              </button>
            )}
            <button type="button" onClick={onCancel}>
              Отмена
            </button>
          </div>
        </>
      ) : (
        <>
          <p>Выберите доску — она будет вставлена в это место заметки.</p>
          <ul aria-label="Доски vault">
            {boards.map((board) => (
              <li key={board.path}>
                <button
                  type="button"
                  disabled={isInserting}
                  onClick={() => onInsert(board.path)}
                >
                  {board.name}
                </button>
              </li>
            ))}
          </ul>
          <div role="group" aria-label="Что сделать">
            <button type="button" onClick={onCancel}>
              Отмена
            </button>
          </div>
        </>
      )}
    </div>
  );
}
