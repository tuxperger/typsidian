import { useCallback, useEffect, useState } from "react";
import type { VaultClient } from "../vault-client.js";
import type { Phrase, Translate } from "../i18n/index.js";
import type {
  AssetsPlacement,
  CommandId,
  SettingsChange,
  Shortcut,
  TemplateSourceEntry,
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
  error: Phrase | null;
  setAssetsPlacement: (placement: AssetsPlacement) => Promise<void>;
  /**
   * Ходить ли в сеть за недостающими шаблонами (фича 007, FR-009).
   *
   * Выключатель, а не включатель: автозагрузка — основное поведение, и
   * настройка существует, чтобы её прекратить.
   */
  setAutoDownloadTemplates: (enabled: boolean) => Promise<void>;
  /**
   * Адреса архивов для конкретных шаблонов (фича 007, FR-004): весь список
   * целиком. Годность пары проверяет ядро при записи — негодный адрес
   * отклоняется с названной причиной, и файл не меняется.
   */
  setTemplateSources: (sources: TemplateSourceEntry[]) => Promise<boolean>;
  /**
   * Папка экспорта в PDF (фича 009, FR-028). `null` — вернуть умолчание
   * `export`. Годность и нормальную форму проверяет ядро при записи: негодная
   * папка отклоняется с причиной, и прежнее значение остаётся (FR-029).
   */
  setExportFolder: (folder: string | null) => Promise<boolean>;
  /**
   * Применяет к перекрытиям vault набор изменений, который ядро направило сюда
   * (фича 006, FR-029).
   *
   * Принимает тот же `SettingsChange`, что и хранилище устройства: две формы
   * набора изменений разошлись бы при первом же новом поле.
   */
  applyChange: (change: SettingsChange) => Promise<void>;
  /**
   * Размер шрифта редактора для этой папки заметок (фича 004, FR-015; фича
   * 006, FR-029).
   *
   * Отдельно от [`applyChange`], хотя поле то же: набор `SettingsChange`
   * собирается из wasm-модуля, а он на desktop и Android не загружается вовсе.
   * Панель настроек папки обязана обходиться без него — иначе размер снова
   * оказался бы настройкой одной только web-сборки.
   *
   * `null` — снять перекрытие: размер вернётся к значению устройства, а там,
   * где хранилища устройства нет, — к запасному значению из стилей.
   */
  setEditorFontSize: (size: number | null) => Promise<void>;
  markSection: (folder: string) => Promise<void>;
  unmarkSection: (folder: string) => Promise<void>;
  /**
   * Назначает команде сочетание либо снимает его при `null`.
   *
   * Снять — не то же, что вернуть умолчание: команда остаётся в палитре, но
   * клавишами не вызывается.
   */
  setKeybinding: (command: CommandId, shortcut: Shortcut | null) => Promise<void>;
  /** Возвращает команду к умолчанию ядра. */
  resetKeybinding: (command: CommandId) => Promise<void>;
  reload: () => Promise<void>;
}

export function useVaultSettings(
  client: VaultClient | null,
): VaultSettingsState {
  const [overrides, setOverrides] = useState<VaultOverrides | null>(null);
  const [error, setError] = useState<Phrase | null>(null);

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
      setError(() => describe(cause));
    }
  }, [client]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const write = useCallback(
    async (next: VaultOverrides) => {
      if (client === null) return false;
      try {
        await client.setVaultSettings(next);
        setOverrides(next);
        setError(null);
        return true;
      } catch (cause) {
        // Состояние не двигается: показать выбор применённым, когда запись
        // отклонена, значило бы соврать пользователю о том, что лежит на
        // диске.
        //
        // Причина ставится **после** перечитывания: удачное чтение сбрасывает
        // ошибку, и в обратном порядке отказ записи исчезал, не показавшись.
        await reload();
        setError(() => describe(cause));
        return false;
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

  const setExportFolder = useCallback(
    async (folder: string | null) => {
      if (overrides === null) return false;
      const written = await write({ ...overrides, export_folder: folder ?? undefined });
      // Ядро хранит папку в нормальной форме (`pdf\2026/` → `pdf/2026`):
      // показать надо записанное, а не набранное.
      if (written) await reload();
      return written;
    },
    [overrides, write, reload],
  );

  const setTemplateSources = useCallback(
    async (sources: TemplateSourceEntry[]) => {
      if (overrides === null) return false;
      return write({ ...overrides, template_sources: sources });
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

  const setEditorFontSize = useCallback(
    async (size: number | null) => {
      if (overrides === null) return;
      // `undefined` в перекрытии — это «спросить устройство», и снятие размера
      // выражается именно им, а не нулём: ключ уходит из файла целиком.
      await write({ ...overrides, editor_font_size: size ?? undefined });
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
        setError(() => describe(cause));
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

  // Сочетания идут отдельными вызовами ядра, а не через `write`: проверку
  // годности и нормализацию делает ядро, и собрать набор здесь значило бы
  // записать в файл то, что оно бы отклонило.
  const setKeybinding = useCallback(
    async (command: CommandId, shortcut: Shortcut | null) => {
      if (client === null) return;
      try {
        setOverrides(await client.setKeybinding(command, shortcut));
        setError(null);
      } catch (cause) {
        setError(() => describeKeybinding(cause));
      }
    },
    [client],
  );

  const resetKeybinding = useCallback(
    async (command: CommandId) => {
      if (client === null) return;
      try {
        setOverrides(await client.resetKeybinding(command));
        setError(null);
      } catch (cause) {
        setError(() => describe(cause));
      }
    },
    [client],
  );

  return {
    overrides,
    error,
    setAssetsPlacement,
    setAutoDownloadTemplates,
    setTemplateSources,
    setExportFolder,
    applyChange,
    setEditorFontSize,
    markSection,
    unmarkSection,
    setKeybinding,
    resetKeybinding,
    reload,
  };
}

/**
 * Отказ назначить сочетание. Негодное сочетание и отказ диска — разные вещи и
 * для пользователя: первое чинится другой клавишей, второе — нет.
 */
function describeKeybinding(cause: unknown): Phrase {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const failure = cause as { kind: string; reason?: { kind?: string } };
    if (failure.kind === "Invalid") {
      switch (failure.reason?.kind) {
        case "NoModifier":
          return (t) => t("settings.keys.noModifier");
        case "ReservedKey":
          return (t) => t("settings.keys.reservedKey");
        case "NoKey":
          return (t) => t("settings.keys.noKey");
        default:
          return (t) => t("settings.keys.invalid");
      }
    }
    if (failure.kind === "Storage") {
      return describe(failure.reason ?? cause);
    }
  }
  return describe(cause);
}

/** Причина отказа словами ядра; общего «что-то пошло не так» здесь нет. */
function describe(cause: unknown): Phrase {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    switch ((cause as { kind: string }).kind) {
      case "Unreadable":
        return (t) => t("settings.vaultStore.unreadable");
      case "NotWritable":
        return (t) => t("settings.vaultStore.notWritable");
      case "NoVaultOpen":
        return (t) => t("settings.vaultStore.noVaultOpen");
      case "InvalidTemplateSource":
        {
        const spec = (cause as { spec?: string }).spec ?? "";
        return (t) => t("settings.vaultStore.invalidTemplateSource", { spec });
      }
      case "InvalidExportFolder":
        {
        const problem = (cause as { problem?: { kind?: string } }).problem?.kind;
        return (t) =>
          t("settings.vaultStore.invalidExportFolder", {
            problem: describeExportFolderProblem(t, problem),
          });
      }
      default:
        break;
    }
  }
  return (t) => t("settings.vaultStore.failed");
}

/** Почему папка экспорта не годится — словами (009 FR-029). */
function describeExportFolderProblem(t: Translate, kind: string | undefined): string {
  switch (kind) {
    case "Empty":
      return t("settings.exportFolder.empty");
    case "Absolute":
      return t("settings.exportFolder.absolute");
    case "LeavesVault":
      return t("settings.exportFolder.leavesVault");
    case "ServiceFolder":
      return t("settings.exportFolder.serviceFolder");
    default:
      return t("settings.exportFolder.invalid");
  }
}
