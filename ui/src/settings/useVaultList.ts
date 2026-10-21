import { useCallback, useState } from "react";
import type { DeviceSettingsClient } from "./device-settings-client.js";
import { changeOf } from "./useSettings.js";
import type { Translate } from "../i18n/index.js";
import type { RejectReason, StoreError } from "../wasm-core/typsidian_core.js";

// Принцип I: ни одна функция здесь не решает, «та ли это папка» и можно ли
// переключаться. Выбор папки — жест пользователя, который обязан произойти в
// UI; всё остальное решает ядро, а этот хук передаёт ему результат жеста и
// исполняет ответ.

/** Отказ операции над списком vault, пригодный к показу пользователю. */
export type VaultActionError =
  | { kind: "PickerCancelled" }
  | { kind: "NotSupported" }
  | { kind: "PermissionDenied" }
  /** Ядро отклонило переключение — вид отказа из плана. */
  | { kind: "SwitchBlocked"; reason: RejectReason["kind"] }
  /** Отказ хранилища: известный вид либо техническое описание как есть. */
  | { kind: "Failed"; store: StoreError["kind"] }
  | { kind: "Unknown"; detail: string };

export function describeVaultActionError(
  t: Translate,
  error: VaultActionError,
): string | null {
  switch (error.kind) {
    case "PickerCancelled":
      // Пользователь передумал — это не сбой.
      return null;
    case "NotSupported":
      return t("settings.vaults.notSupported");
    case "PermissionDenied":
      return t("settings.vaults.permissionDenied");
    case "SwitchBlocked":
      return describeReject(t, error.reason);
    case "Failed":
      return describeStoreError(t, error.store);
    case "Unknown":
      return t("settings.vaults.failed", { detail: error.detail });
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
  client: DeviceSettingsClient,
  options: {
    onChanged: () => void | Promise<void>;
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
  const { onChanged, confirmSwitch, onSwitched } = options;

  const addFolder = useCallback(async () => {
    setError(null);
    try {
      // Жест выбора папки у каждой платформы свой, и делает его клиент. Ссылку
      // на папку ядро получает не разобранной: повторное добавление той же
      // папки оно само сведёт к существующей записи (FR-020).
      const outcome = await client.addFolder();
      if (outcome.kind === "unsupported") {
        setError({ kind: "NotSupported" });
        return;
      }
      if (outcome.kind === "added") await onChanged();
    } catch (cause) {
      setError(classify(cause));
    }
  }, [client, onChanged]);

  const switchTo = useCallback(
    async (id: string) => {
      setError(null);
      // Предупреждения о несохранённых правках и конфликтах — существующая
      // машинерия фичи 001, а не новая проверка (FR-022).
      if (confirmSwitch) {
        const allowed = await confirmSwitch(id);
        if (!allowed) return;
      }
      try {
        const { plan } = await client.apply(changeOf({ current_vault: id }));
        if (plan.outcome === "Rejected") {
          setError({ kind: "SwitchBlocked", reason: plan.reason.kind });
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
    [client, onChanged, confirmSwitch, onSwitched],
  );

  const setLabel = useCallback(
    async (id: string, label: string | null) => {
      setError(null);
      try {
        await client.setVaultLabel(id, label);
        await onChanged();
      } catch (cause) {
        setError(classify(cause));
      }
    },
    [client, onChanged],
  );

  const forget = useCallback(
    async (id: string) => {
      setError(null);
      try {
        // FR-024: убирается запись и ссылка на неё; папка и заметки на диске
        // не трогаются — ядро к ним и не обращается.
        await client.forgetVault(id);
        await onChanged();
      } catch (cause) {
        setError(classify(cause));
      }
    },
    [client, onChanged],
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
  // Отказ хранилища приходит объектом `StoreError` — с командой Tauri так же,
  // как с wasm-bindgen; `String()` превратил бы его в «[object Object]».
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const { kind } = cause;
    if (kind === "NoConfigDirectory" || kind === "Io" || kind === "RevealUnsupported") {
      return { kind: "Failed", store: kind };
    }
    return { kind: "Unknown", detail: String(kind) };
  }
  return { kind: "Unknown", detail: String(cause) };
}

function describeReject(t: Translate, kind: RejectReason["kind"]): string {
  switch (kind) {
    case "ValueOutOfRange":
      return t("settings.vaults.unknownRecord");
    case "StorageNotWritable":
      return t("settings.reject.storageNotWritable");
    default: {
      const exhaustive: never = kind;
      return exhaustive;
    }
  }
}

function describeStoreError(t: Translate, kind: StoreError["kind"]): string {
  switch (kind) {
    case "NoConfigDirectory":
      return t("settings.store.noConfigDirectory");
    case "Io":
      return t("settings.store.io");
    case "RevealUnsupported":
      return t("settings.store.revealUnsupported");
    default: {
      const exhaustive: never = kind;
      return exhaustive;
    }
  }
}
