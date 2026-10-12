import { useCallback, useState } from "react";
import type { DiffLine, ResolveError } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import type { Translate } from "../i18n/index.js";

// Принцип I: решение «есть ли конфликт», «что именно расходится» и «что
// отбросить» принимает ядро (`ConflictRegistry`). Этот хук только вызывает
// `resolveConflictKeepLocal`/`resolveConflictAcceptExternal`/`conflictDiff`
// и хранит то, что нужно исключительно для отрисовки.

/**
 * Как заметка попала в конфликтное состояние. Различие видно пользователю:
 * при `deleted` внешней версии не существует и вариант «принять внешнюю»
 * означает принять удаление (FR-025), а не подставить чужой текст.
 */
export type ConflictKind = "diskChanged" | "deleted";

export interface ConflictEntry {
  notePath: string;
  kind: ConflictKind;
}

export interface ConflictResolutionState {
  /** Активные конфликты в порядке появления; первый показывается диалогом. */
  conflicts: ConflictEntry[];
  /** Отличия для текущего конфликта — `null`, пока пользователь их не запросил. */
  diff: DiffLine[] | null;
  isResolving: boolean;
  error: ResolveError | null;
}

const INITIAL_STATE: ConflictResolutionState = {
  conflicts: [],
  diff: null,
  isResolving: false,
  error: null,
};

// Отклонённый промис приходит из WASM как размеченное объединение, но
// сгенерированный `.d.ts` объявляет метод как `Promise<void>` — тип причины
// отказа на стороне TS не виден. Guard восстанавливает точный тип без `any`
// и без приведения (тот же приём, что и в `VaultTreeActions.tsx`).
function asResolveError(cause: unknown): ResolveError | null {
  if (typeof cause !== "object" || cause === null || !("kind" in cause)) {
    return null;
  }
  const { kind } = cause;
  if (kind === "NoPendingConflict") return { kind };
  if (kind === "AccessDenied") return { kind };
  if (kind === "Io") return { kind };
  return null;
}

export function describeResolveError(t: Translate, error: ResolveError): string {
  switch (error.kind) {
    case "NoPendingConflict":
      return t("editor.conflict.resolve.noPending");
    case "AccessDenied":
      return t("editor.conflict.resolve.accessDenied");
    case "Io":
      return t("editor.conflict.resolve.io");
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

/**
 * Связывает диалог конфликта с операциями разрешения в ядре (T078, FR-021…
 * FR-025).
 *
 * Ключевое обязательство: ни один путь здесь не отбрасывает буфер сам.
 * «Принять внешнюю» вызывается только из обработчика соответствующей кнопки,
 * то есть после явного действия пользователя (FR-023); закрытие диалога
 * оставляет конфликт неразрешённым, чтобы его подхватил
 * `useWindowCloseGuard` (FR-037).
 */
export function useConflictResolution(
  client: VaultClient | null,
  /**
   * Путь вкладки, чей конфликт показывается диалогом (фича 014, research.md §7):
   * остальные ждут с признаком на своей вкладке. `undefined` — выбора нет, как
   * раньше показывается первый по времени появления.
   */
  focusPath?: string | null,
): {
  state: ConflictResolutionState;
  current: ConflictEntry | null;
  noteConflicted: (notePath: string, kind: ConflictKind) => void;
  /** Заметки перенесены средствами приложения: конфликт едет за путём (023, FR-020). */
  pathsMoved: (changes: readonly { from: string; to: string }[]) => void;
  showDiff: () => Promise<void>;
  hideDiff: () => void;
  keepLocal: () => Promise<void>;
  acceptExternal: () => Promise<string | null>;
  dismissError: () => void;
} {
  const [state, setState] = useState<ConflictResolutionState>(INITIAL_STATE);
  const current =
    focusPath === undefined
      ? (state.conflicts[0] ?? null)
      : (state.conflicts.find((entry) => entry.notePath === focusPath) ?? null);

  const noteConflicted = useCallback((notePath: string, kind: ConflictKind) => {
    setState((prev) => {
      const existing = prev.conflicts.find((c) => c.notePath === notePath);
      if (existing) {
        // Повторное внешнее изменение до разрешения не создаёт второй записи —
        // ядро актуализирует тот же конфликт (US4 AS5). Но «изменён» может
        // смениться на «удалён»: это более сильный факт.
        if (existing.kind === kind) return prev;
        return {
          ...prev,
          conflicts: prev.conflicts.map((c) =>
            c.notePath === notePath ? { notePath, kind } : c,
          ),
          diff: null,
        };
      }
      return { ...prev, conflicts: [...prev.conflicts, { notePath, kind }] };
    });
  }, []);

  const currentPath = current?.notePath ?? null;
  const pathsMoved = useCallback((changes: readonly { from: string; to: string }[]) => {
    if (changes.length === 0) return;
    const moved = new Map(changes.map((change) => [change.from, change.to]));
    setState((prev) => {
      if (!prev.conflicts.some((c) => moved.has(c.notePath))) return prev;
      return {
        ...prev,
        conflicts: prev.conflicts.map((c) => ({
          ...c,
          notePath: moved.get(c.notePath) ?? c.notePath,
        })),
      };
    });
  }, []);

  const dropCurrent = useCallback(() => {
    setState((prev) => ({
      ...prev,
      conflicts: prev.conflicts.filter((entry) => entry.notePath !== currentPath),
      diff: null,
      isResolving: false,
    }));
  }, [currentPath]);

  const showDiff = useCallback(async () => {
    if (!client || !current) return;
    const diff = await client.conflictDiff(current.notePath);
    setState((prev) => ({ ...prev, diff }));
  }, [client, current]);

  const hideDiff = useCallback(() => {
    setState((prev) => ({ ...prev, diff: null }));
  }, []);

  const keepLocal = useCallback(async () => {
    if (!client || !current || state.isResolving) return;
    setState((prev) => ({ ...prev, isResolving: true, error: null }));
    try {
      await client.resolveConflictKeepLocal(current.notePath);
      dropCurrent();
    } catch (cause) {
      setState((prev) => ({
        ...prev,
        isResolving: false,
        error: asResolveError(cause) ?? { kind: "Io" },
      }));
    }
  }, [client, current, state.isResolving, dropCurrent]);

  /**
   * Возвращает содержимое, которое редактор обязан показать вместо буфера,
   * либо `null`, если внешней версии нет (заметка удалена снаружи — принять
   * внешнюю значит принять удаление, FR-025).
   */
  const acceptExternal = useCallback(async (): Promise<string | null> => {
    if (!client || !current || state.isResolving) return null;
    setState((prev) => ({ ...prev, isResolving: true, error: null }));
    try {
      const external = await client.resolveConflictAcceptExternal(
        current.notePath,
      );
      dropCurrent();
      return external ?? null;
    } catch (cause) {
      setState((prev) => ({
        ...prev,
        isResolving: false,
        error: asResolveError(cause) ?? { kind: "Io" },
      }));
      return null;
    }
  }, [client, current, state.isResolving, dropCurrent]);

  const dismissError = useCallback(() => {
    setState((prev) => ({ ...prev, error: null }));
  }, []);

  return {
    state,
    current,
    noteConflicted,
    pathsMoved,
    showDiff,
    hideDiff,
    keepLocal,
    acceptExternal,
    dismissError,
  };
}
