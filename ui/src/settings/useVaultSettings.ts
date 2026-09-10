import { useCallback, useEffect, useState } from "react";
import type { VaultClient } from "../vault-client.js";
import type {
  AssetsPlacement,
  SettingsChange,
  VaultOverrides,
} from "../wasm-core/typsidian_core.js";

/**
 * Настройки уровня vault (фича 006, FR-002, FR-015).
 *
 * Отдельный хук, а не расширение `useSettings`: тот работает с хранилищем
 * **устройства** (`SettingsStore`), а эти значения живут внутри открытого
 * vault и читаются через `VaultClient`. Смешать их значило бы сцепить два
 * независимых хранилища в одном месте — ровно то, чего избегает ядро,
 * положив настройки vault функциями поверх `VaultBackend`, а не третьей
 * реализацией `SettingsStore`.
 *
 * Ни одного решения о том, куда попадёт файл вставки, здесь нет: хук читает и
 * пишет значения, а расчёт пути принадлежит ядру (принцип I).
 */
export interface VaultSettingsState {
  /** `null` — ещё не прочитано либо vault не открыт. */
  overrides: VaultOverrides | null;
  /** Причина отказа последней операции, названная ядром. */
  error: string | null;
  setAssetsPlacement: (placement: AssetsPlacement) => Promise<void>;
  /**
   * Ходить ли в сеть за недостающими шаблонами (фича 007, FR-009).
   *
   * Выключатель, а не включатель: автозагрузка — основное поведение, и
   * настройка существует, чтобы её прекратить.
   */
  setAutoDownloadTemplates: (enabled: boolean) => Promise<void>;
  /**
   * Применяет к перекрытиям vault набор изменений, который ядро направило сюда
   * (фича 006, FR-029).
   *
   * Принимает тот же `SettingsChange`, что и хранилище устройства: две формы
   * набора изменений разошлись бы при первом же новом поле.
   */
  applyChange: (change: SettingsChange) => Promise<void>;
  markSection: (folder: string) => Promise<void>;
  unmarkSection: (folder: string) => Promise<void>;
  reload: () => Promise<void>;
}

export function useVaultSettings(
  client: VaultClient | null,
): VaultSettingsState {
  const [overrides, setOverrides] = useState<VaultOverrides | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    if (client === null) {
      setOverrides(null);
      return;
    }
    try {
      setOverrides(await client.vaultSettings());
      setError(null);
    } catch (cause) {
      // Причина показывается, но не придумывается: у отказа записи в
      // недоступный на запись vault и у испорченного файла разные последствия
      // для пользователя (FR-025, FR-034).
      setError(describe(cause));
    }
  }, [client]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const write = useCallback(
    async (next: VaultOverrides) => {
      if (client === null) return;
      try {
        await client.setVaultSettings(next);
        setOverrides(next);
        setError(null);
      } catch (cause) {
        setError(describe(cause));
        // Состояние не двигается: показать выбор применённым, когда запись
        // отклонена, значило бы соврать пользователю о том, что лежит на
        // диске.
        await reload();
      }
    },
    [client, reload],
  );

  const setAutoDownloadTemplates = useCallback(
    async (enabled: boolean) => {
      if (overrides === null) return;
      await write({ ...overrides, auto_download_templates: enabled });
    },
    [overrides, write],
  );

  const setAssetsPlacement = useCallback(
    async (placement: AssetsPlacement) => {
      if (overrides === null) return;
      await write({ ...overrides, assets_placement: placement });
    },
    [overrides, write],
  );

  const applyChange = useCallback(
    async (change: SettingsChange) => {
      if (overrides === null) return;
      // `undefined` в наборе означает «не трогать» — ровно то же, что и в
      // хранилище устройства; здесь оно превращается в «оставить прежнее
      // перекрытие», а не в «сбросить к значению устройства».
      await write({
        ...overrides,
        ...(change.theme !== undefined ? { theme: change.theme } : {}),
        ...(change.editor_font_size !== undefined
          ? { editor_font_size: change.editor_font_size }
          : {}),
        ...(change.editor_mode_preference !== undefined
          ? { editor_mode_preference: change.editor_mode_preference }
          : {}),
        ...(change.editor_config_path !== undefined
          ? { editor_config_path: change.editor_config_path }
          : {}),
      });
    },
    [overrides, write],
  );

  const toggleSection = useCallback(
    async (folder: string, marked: boolean) => {
      if (client === null) return;
      try {
        // Обновлённый набор возвращает ядро — второй запрос за тем, что оно
        // уже посчитало, был бы лишним кругом через границу.
        setOverrides(
          marked
            ? await client.markSection(folder)
            : await client.unmarkSection(folder),
        );
        setError(null);
      } catch (cause) {
        setError(describe(cause));
      }
    },
    [client],
  );

  const markSection = useCallback(
    (folder: string) => toggleSection(folder, true),
    [toggleSection],
  );
  const unmarkSection = useCallback(
    (folder: string) => toggleSection(folder, false),
    [toggleSection],
  );

  return {
    overrides,
    error,
    setAssetsPlacement,
    setAutoDownloadTemplates,
    applyChange,
    markSection,
    unmarkSection,
    reload,
  };
}

/** Причина отказа словами ядра; общего «что-то пошло не так» здесь нет. */
function describe(cause: unknown): string {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    switch ((cause as { kind: string }).kind) {
      case "Unreadable":
        return "Файл настроек vault не разбирается. Он не перезаписан — поправьте его вручную.";
      case "NotWritable":
        return "Vault недоступен на запись: настройки уровня vault сохранить некуда.";
      case "NoVaultOpen":
        return "Vault не открыт.";
      default:
        break;
    }
  }
  return "Настройки vault прочитать или записать не удалось.";
}
