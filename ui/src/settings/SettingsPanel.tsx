import { useState } from "react";
import type {
  ApplyPlan,
  ModePreference,
  OutageReason,
} from "../wasm-core/typsidian_core.js";
import { AdvancedSection } from "./AdvancedSection.js";
import { AppearanceSection } from "./AppearanceSection.js";
import { useAppearance } from "./applyToDocument.js";
import { applyWithNoteSave, describeApplyOutcome } from "./applyWithNoteSave.js";
import { KeybindingsSection } from "./KeybindingsSection.js";
import {
  changeOf,
  useSettings,
  type SettingsState,
  type StorageNotice,
} from "./useSettings.js";
import { describeVaultActionError, useVaultList, type ConfirmSwitch } from "./useVaultList.js";
import { VaultSection } from "./VaultSection.js";

/**
 * Панель настроек: четыре раздела и **ни одной кнопки «сохранить» или
 * «применить»** (T039, FR-001, FR-003). Отсутствие этих кнопок — не упущение
 * каркаса, а требование: изменение вступает в силу немедленно.
 *
 * Единственное исключение — «Применить» у конфига режима редактирования: там
 * это не сохранение настройки, а явное действие, без которого конфиг не
 * перечитывается (FR-026).
 */
export interface SettingsPanelProps {
  /**
   * Предупреждение перед переключением vault (FR-022, SC-008). Передаётся
   * владельцем vault-бэкенда: машинерия несохранённых правок и неразрешённых
   * конфликтов живёт в фиче 001 и здесь не переизобретается.
   */
  confirmSwitch?: ConfirmSwitch;
  /**
   * Сохранение открытой заметки существующим `VaultBackend::write` — исполнение
   * инструкции `RequiresNoteSave`, вернувшейся из ядра (FR-029, research.md §9).
   * Без него смена режима при открытой заметке с правками не завершается.
   */
  saveNote?: (path: string) => Promise<boolean>;
  /**
   * Почему полноценный режим недоступен на этом устройстве (FR-034), либо
   * `null`, если доступен. Решение принимает ядро —
   * `EditorModeController::availability()`; панель его только отображает
   * (принцип I). Значение инъецируется хозяином панели по той же причине, что
   * `confirmSwitch` и `saveNote`: `WebStore` ещё не отдаёт `BridgeAvailability`
   * через свою wasm-границу, и придумывать ответ в UI вместо ядра нельзя.
   */
  fullBackendUnavailableReason?: OutageReason | null;
}

export function SettingsPanel({
  confirmSwitch,
  saveNote,
  fullBackendUnavailableReason = null,
}: SettingsPanelProps = {}): React.JSX.Element {
  const { state, apply, resetToDefaults, refresh, dismissNotice } = useSettings();
  const vaults = useVaultList(state.store, {
    onChanged: refresh,
    ...(confirmSwitch ? { confirmSwitch } : {}),
  });
  const [modeMessage, setModeMessage] = useState<string | null>(null);
  const [resetMessage, setResetMessage] = useState<string | null>(null);

  /**
   * Сброс к умолчаниям (FR-037). Отказ ядра показывается словами: сброс,
   * молча не состоявшийся, неотличим для пользователя от сброса, вернувшего
   * те же значения.
   */
  const runReset = async (): Promise<void> => {
    const plan = await resetToDefaults();
    if (!plan) return;
    setResetMessage(
      plan.outcome === "Applied"
        ? "Значения вернулись к умолчаниям. Список известных vault не изменился."
        : describeResetRejection(plan),
    );
  };

  /**
   * Смена режима идёт через двухшаговый разбор плана: ядро может потребовать
   * сначала сохранить заметку (FR-029). Если исполнитель сохранения не
   * передан, требование честно превращается в сообщение — молча менять режим,
   * потеряв правки, нельзя.
   */
  const changeMode = async (preference: ModePreference): Promise<void> => {
    const change = changeOf({ editor_mode_preference: preference });
    const outcome = await applyWithNoteSave(change, {
      apply,
      saveNote:
        saveNote ??
        (async () => {
          setModeMessage(
            "Сначала сохраните открытую заметку — смена режима требует этого.",
          );
          return false;
        }),
    });
    if (outcome) setModeMessage(describeApplyOutcome(outcome));
  };

  // FR-017, SC-002: тема и размер шрифта доходят до остального интерфейса через
  // CSS-переменные на корне документа — в том числе когда значения изменились
  // снаружи, а не через панель.
  useAppearance(state.settings);

  if (state.isLoading || !state.settings) {
    return <p role="status">Загружаем настройки…</p>;
  }

  const settings = state.settings;

  return (
    <div aria-label="Настройки">
      {state.notices.map((notice) => (
        <StorageNoticeBanner
          key={notice.kind}
          notice={notice}
          onDismiss={() => dismissNotice(notice.kind)}
        />
      ))}

      <AppearanceSection
        settings={settings}
        onChangeTheme={(theme) => void apply(changeOf({ theme }))}
        onChangeFontSize={(size) => void apply(changeOf({ editor_font_size: size }))}
        rejectionMessage={describeRejection(state)}
      />

      <VaultSection
        knownVaults={state.knownVaults}
        currentVaultId={settings.current_vault ?? null}
        onAddFolder={vaults.addFolder}
        onSwitch={vaults.switchTo}
        onSetLabel={vaults.setLabel}
        onForget={vaults.forget}
        errorMessage={vaults.error ? describeVaultActionError(vaults.error) : null}
      />

      <KeybindingsSection
        mode={state.editorMode}
        configDiskState={state.configDiskState}
        configPath={settings.editor_config_path ?? null}
        fullBackendUnavailableReason={fullBackendUnavailableReason}
        onChooseFullBackend={() => void changeMode("fullBackend")}
        onChooseSimplifiedEmulation={() => void changeMode("simplifiedEmulation")}
        onPickConfig={() => {}}
        onApplyConfig={() => {}}
        onLeavePlainInput={() => {}}
      />

      {/* FR-029: о вынужденном сохранении заметки и об отказе сменить режим
          пользователь узнаёт словами, а не по молчаливому бездействию. */}
      {modeMessage && <p role="status">{modeMessage}</p>}

      <AdvancedSection
        capabilities={state.capabilities}
        diagnostics={state.diagnostics}
        onReveal={() => state.store?.reveal()}
        onResetToDefaults={() => void runReset()}
        resetMessage={resetMessage}
      />
    </div>
  );
}

/**
 * Причина отказа берётся из плана, вернувшегося из ядра (FR-016): UI
 * сопоставляет вариантам формулировки, но не решает сам, что допустимо.
 */
function describeRejection(state: SettingsState): string | null {
  const plan = state.lastPlan;
  if (!plan || plan.outcome !== "Rejected") return null;
  switch (plan.reason.kind) {
    case "ValueOutOfRange":
      return "Значение вне допустимого диапазона — размер шрифта должен быть от 8 до 32.";
    case "SaveFailed":
      return "Не удалось сохранить открытую заметку, поэтому режим не изменён. Правки не потеряны.";
    case "UnresolvedConflict":
      return "Сначала нужно разрешить конфликт по открытой заметке.";
    case "StorageNotWritable":
      return "Хранилище настроек недоступно на запись: изменение действует до конца сессии.";
    default: {
      const exhaustive: never = plan.reason;
      return exhaustive;
    }
  }
}

/**
 * Почему сброс не состоялся (FR-037). Формулировки отличаются от общих: при
 * сбросе важно сказать, что **ни одно** значение не изменилось, — иначе
 * пользователь заподозрит частичное применение.
 */
function describeResetRejection(plan: ApplyPlan): string {
  if (plan.outcome !== "Rejected") return "Сброс не выполнен.";
  switch (plan.reason.kind) {
    case "UnresolvedConflict":
      return "Сброс отменён целиком: сначала нужно разрешить конфликт по открытой заметке. Ни одна настройка не изменилась.";
    case "SaveFailed":
      return "Сброс отменён целиком: не удалось сохранить открытую заметку. Ни одна настройка не изменилась, правки не потеряны.";
    case "StorageNotWritable":
      return "Хранилище недоступно на запись: сброс действует до конца сессии и не переживёт перезапуск.";
    case "ValueOutOfRange":
      // Недостижимо: набор собирает ядро из своих же умолчаний. Ветка
      // существует ради исчерпывающей проверки (принцип IV).
      return "Сброс отклонён ядром как недопустимый набор значений.";
    default: {
      const exhaustive: never = plan.reason;
      return exhaustive;
    }
  }
}

/** Состояния хранилища, о которых пользователь должен знать (T042). */
function StorageNoticeBanner({
  notice,
  onDismiss,
}: {
  notice: StorageNotice;
  onDismiss: () => void;
}): React.JSX.Element {
  return (
    <div role="status">
      <p>{describeNotice(notice)}</p>
      <button type="button" onClick={onDismiss}>
        Понятно
      </button>
    </div>
  );
}

function describeNotice(notice: StorageNotice): string {
  switch (notice.kind) {
    case "unreadable":
      // FR-013: подчёркиваем, что файл пользователя не тронут — иначе
      // сообщение читается как «мы его починили за вас».
      return "Файл настроек не читается — работаем на значениях по умолчанию. Ваш файл не тронут.";
    case "readOnly":
      return "Каталог настроек недоступен на запись: изменения действуют до конца сессии и не переживут перезапуск.";
    case "invalidSetAside":
      return `Испорченный файл настроек перенесён рядом: ${notice.movedTo ?? "—"}.`;
    default: {
      const exhaustive: never = notice.kind;
      return exhaustive;
    }
  }
}
