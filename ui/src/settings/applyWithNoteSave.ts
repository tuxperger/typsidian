import type { ApplyPlan, SettingsChange } from "../wasm-core/typsidian_core.js";

// contracts/frontend-settings.md: разбор `ApplyPlan` — единственное место, где
// UI делает больше одного шага. И это именно **исполнение инструкции**,
// вернувшейся из ядра, а не самостоятельное решение: UI не проверяет сам, есть
// ли конфликт и нужно ли сохранять (принцип I).

/** Чем закончилось применение с точки зрения пользователя. */
export type ApplyOutcome =
  | { kind: "applied" }
  /** Заметка была сохранена по требованию ядра — об этом MUST сообщить (FR-029). */
  | { kind: "appliedAfterSavingNote"; notePath: string }
  /** Сохранение не удалось: режим не меняется, правки не потеряны (FR-030). */
  | { kind: "saveFailed"; notePath: string }
  /** Ядро отказало: ни одна настройка не изменилась. */
  | { kind: "rejected"; plan: Extract<ApplyPlan, { outcome: "Rejected" }> };

export interface ApplyWithNoteSaveDeps {
  apply: (change: SettingsChange) => Promise<ApplyPlan | null>;
  /**
   * Сохранение заметки существующим примитивом `VaultBackend::write`.
   * Возвращает `true` при успехе. Настройки к vault-бэкенду не обращаются —
   * связывать две независимые подсистемы означало бы сцепить их (research.md
   * §9), поэтому исполнитель передаётся снаружи.
   */
  saveNote: (path: string) => Promise<boolean>;
}

/**
 * Применяет изменение, исполняя `RequiresNoteSave`, если ядро его вернуло
 * (T084, FR-029, FR-030).
 *
 * ```text
 * apply ─▶ Applied            ─▶ перерисовать
 *       ─▶ RequiresNoteSave{p} ─▶ saveNote(p)
 *                                 ├─ успех ─▶ сообщить о сохранении, повторить apply
 *                                 └─ отказ ─▶ показать причину; режим НЕ менять
 *       ─▶ Rejected{reason}    ─▶ показать причину; НИ ОДНА настройка не меняется
 * ```
 *
 * Повтор ровно один: если ядро снова требует сохранения после успешной записи,
 * значит заметка изменилась опять — крутить этот цикл значило бы драться с
 * пользователем за его же буфер. Второй `RequiresNoteSave` трактуется как
 * неудача сохранения, и режим не меняется.
 */
export async function applyWithNoteSave(
  change: SettingsChange,
  { apply, saveNote }: ApplyWithNoteSaveDeps,
): Promise<ApplyOutcome | null> {
  const first = await apply(change);
  if (!first) return null;

  switch (first.outcome) {
    case "Applied":
      return { kind: "applied" };
    case "Rejected":
      return { kind: "rejected", plan: first };
    case "RequiresNoteSave": {
      const notePath = first.path;
      const saved = await saveNote(notePath);
      if (!saved) {
        return { kind: "saveFailed", notePath };
      }
      const second = await apply(change);
      if (!second) return null;
      switch (second.outcome) {
        case "Applied":
          return { kind: "appliedAfterSavingNote", notePath };
        case "Rejected":
          return { kind: "rejected", plan: second };
        case "RequiresNoteSave":
          return { kind: "saveFailed", notePath };
        default: {
          const exhaustive: never = second;
          return exhaustive;
        }
      }
    }
    default: {
      const exhaustive: never = first;
      return exhaustive;
    }
  }
}

/** Сообщение пользователю по итогу (FR-029 требует сообщить о сохранении). */
export function describeApplyOutcome(outcome: ApplyOutcome): string | null {
  switch (outcome.kind) {
    case "applied":
      return null;
    case "appliedAfterSavingNote":
      return `Заметка «${outcome.notePath}» сохранена перед сменой режима.`;
    case "saveFailed":
      return `Не удалось сохранить «${outcome.notePath}» — режим не изменён, правки не потеряны.`;
    case "rejected":
      return describeReject(outcome.plan.reason.kind);
    default: {
      const exhaustive: never = outcome;
      return exhaustive;
    }
  }
}

function describeReject(kind: string): string {
  switch (kind) {
    case "UnresolvedConflict":
      return "Сначала нужно разрешить конфликт по открытой заметке — режим не изменён.";
    case "SaveFailed":
      return "Сохранить открытую заметку не удалось — режим не изменён, правки не потеряны.";
    case "ValueOutOfRange":
      return "Значение вне допустимого диапазона.";
    default:
      return "Хранилище настроек недоступно на запись.";
  }
}
