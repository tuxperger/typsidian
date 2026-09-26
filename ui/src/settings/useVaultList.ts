import { useCallback, useState } from "react";
import type { WebStore } from "../wasm-core/typsidian_core.js";
import type { WasmCallQueue } from "../wasm-call-queue.js";

// Принцип I: ни одна функция здесь не решает, «та ли это папка» и можно ли
// переключаться. Выбор папки — жест пользователя, который обязан произойти в
// UI; всё остальное решает ядро, а этот хук передаёт ему результат жеста и
// исполняет ответ.

/** Отказ операции над списком vault, пригодный к показу пользователю. */
export type VaultActionError =
  | { kind: "PickerCancelled" }
  | { kind: "NotSupported" }
  | { kind: "PermissionDenied" }
  | { kind: "SwitchBlocked"; message: string }
  | { kind: "Failed"; message: string };

export function describeVaultActionError(error: VaultActionError): string | null {
  switch (error.kind) {
    case "PickerCancelled":
      // Пользователь передумал — это не сбой.
      return null;
    case "NotSupported":
      return "Этот браузер не умеет открывать локальные папки (нет File System Access API).";
    case "PermissionDenied":
      return "Доступ к папке не предоставлен. Попробуйте добавить её заново и разрешите доступ, когда браузер спросит.";
    case "SwitchBlocked":
      return error.message;
    case "Failed":
      return `Не удалось выполнить операцию: ${error.message}`;
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

/**
 * Предупреждения перед переключением vault (FR-022, SC-008).
 *
 * Тип, а не реализация: машинерия несохранённых правок и неразрешённых
 * конфликтов уже существует в фиче 001 (`unresolvedEditPaths`,
 * `pendingConflict`), и переизобретать её здесь запрещено. Владелец
 * vault-бэкенда передаёт эту проверку сюда; если её не передали, переключение
 * идёт без предупреждения — терять при этом нечему.
 */
export type ConfirmSwitch = (nextVaultId: string) => boolean | Promise<boolean>;

export function useVaultList(
  store: WebStore | null,
  options: {
    onChanged: () => void | Promise<void>;
    /**
     * Очередь доступа к `store` — та же, что у `useSettings`. Методы store'а
     * заимствуют объект ядра на всё время своей future, поэтому звать их в
     * обход очереди значит наложиться на цикл опроса настроек
     * (`wasm-call-queue.ts`).
     */
    queue: WasmCallQueue;
    confirmSwitch?: ConfirmSwitch;
    /**
     * Ядро приняло смену текущего vault (US2 AS3). Сам vault после этого
     * обязан открыться в окне — делает это владелец рабочей области, не этот
     * хук: клиента vault у него нет и заводить его здесь нечего.
     */
    onSwitched?: (id: string) => void;
  },
): {
  error: VaultActionError | null;
  addFolder: () => Promise<void>;
  switchTo: (id: string) => Promise<void>;
  setLabel: (id: string, label: string | null) => Promise<void>;
  forget: (id: string) => Promise<void>;
  dismissError: () => void;
} {
  const [error, setError] = useState<VaultActionError | null>(null);
  const { onChanged, confirmSwitch, onSwitched, queue } = options;

  const addFolder = useCallback(async () => {
    if (!store) return;
    setError(null);
    const picker = (
      window as unknown as {
        showDirectoryPicker?: () => Promise<FileSystemDirectoryHandle>;
      }
    ).showDirectoryPicker;
    if (!picker) {
      setError({ kind: "NotSupported" });
      return;
    }
    try {
      // Жест пользователя: `showDirectoryPicker` нельзя вызвать из ядра —
      // по той же причине, что и в `vault-tree/useVault.ts`.
      const handle = await picker();
      // Ссылка передаётся ядру **не разбирая** её. Повторное добавление той же
      // папки ядро само сведёт к существующей записи (FR-020), а две разные
      // папки с одинаковым именем останутся двумя записями.
      await queue.run(() => store.addVault(handle, handle.name));
      await onChanged();
    } catch (cause) {
      setError(classify(cause));
    }
  }, [store, onChanged, queue]);

  const switchTo = useCallback(
    async (id: string) => {
      if (!store) return;
      setError(null);
      // Предупреждения о несохранённых правках и конфликтах — существующая
      // машинерия фичи 001, а не новая проверка (FR-022).
      if (confirmSwitch) {
        const allowed = await confirmSwitch(id);
        if (!allowed) return;
      }
      try {
        const plan = await queue.run(() =>
          store.apply({
            theme: undefined,
            editor_font_size: undefined,
            current_vault: id,
            editor_mode_preference: undefined,
            editor_config_path: undefined,
          }),
        );
        if (plan.outcome === "Rejected") {
          setError({ kind: "SwitchBlocked", message: describeReject(plan.reason.kind) });
          return;
        }
        await onChanged();
        // Vault переключён — окну пора открыть выбранный (US2 AS3). После
        // `onChanged`, а не вместо него: список обязан показать новое текущее
        // состояние независимо от того, что сделает окно.
        onSwitched?.(id);
      } catch (cause) {
        setError(classify(cause));
      }
    },
    [store, onChanged, confirmSwitch, onSwitched, queue],
  );

  const setLabel = useCallback(
    async (id: string, label: string | null) => {
      if (!store) return;
      setError(null);
      try {
        await queue.run(() => store.setVaultLabel(id, label ?? undefined));
        await onChanged();
      } catch (cause) {
        setError(classify(cause));
      }
    },
    [store, onChanged, queue],
  );

  const forget = useCallback(
    async (id: string) => {
      if (!store) return;
      setError(null);
      try {
        // FR-024: убирается запись и ссылка на неё; папка и заметки на диске
        // не трогаются — ядро к ним и не обращается.
        await queue.run(() => store.forgetVault(id));
        await onChanged();
      } catch (cause) {
        setError(classify(cause));
      }
    },
    [store, onChanged, queue],
  );

  const dismissError = useCallback(() => setError(null), []);

  return { error, addFolder, switchTo, setLabel, forget, dismissError };
}

function classify(cause: unknown): VaultActionError {
  if (cause instanceof DOMException) {
    if (cause.name === "AbortError") return { kind: "PickerCancelled" };
    if (cause.name === "NotAllowedError" || cause.name === "SecurityError") {
      return { kind: "PermissionDenied" };
    }
  }
  return { kind: "Failed", message: String(cause) };
}

function describeReject(kind: string): string {
  switch (kind) {
    case "UnresolvedConflict":
      return "Сначала нужно разрешить конфликт по открытой заметке.";
    case "SaveFailed":
      return "Не удалось сохранить открытую заметку — vault не переключён, правки не потеряны.";
    case "ValueOutOfRange":
      return "Эта запись о vault больше не известна приложению.";
    default:
      return "Хранилище настроек недоступно на запись.";
  }
}
