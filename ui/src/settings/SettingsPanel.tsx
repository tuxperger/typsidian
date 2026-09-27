import { useEffect, useState } from "react";
import type {
  ApplyPlan,
  ModePreference,
  OutageReason,
  Settings,
  SettingsChange,
} from "../wasm-core/typsidian_core.js";
import type { DeviceSettingsClient } from "./device-settings-client.js";
import { AdvancedSection } from "./AdvancedSection.js";
import { AppearanceSection } from "./AppearanceSection.js";
import { useAppearance } from "./applyToDocument.js";
import { applyWithNoteSave, describeApplyOutcome } from "./applyWithNoteSave.js";
import { KeybindingsSection } from "./KeybindingsSection.js";
import { ShortcutsSection } from "./ShortcutsSection.js";
import {
  changeOf,
  useSettings,
  type SettingsState,
  type StorageNotice,
} from "./useSettings.js";
import { describeVaultActionError, useVaultList, type ConfirmSwitch } from "./useVaultList.js";
import { VaultSection } from "./VaultSection.js";
import { useVaultSettings } from "./useVaultSettings.js";
import type { VaultClient } from "../vault-client.js";
import type { PreviewClient } from "../preview-client.js";
import { TemplatesSection } from "./TemplatesSection.js";
import { useShortcuts } from "../workspace/useShortcuts.js";

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
   * Хранилище настроек устройства (фича 004). Приходит от оболочки, потому что
   * оно у каждой своё: `WebStore` поверх OPFS в браузере, `settings.json` в
   * каталоге конфигурации ОС за командами Tauri на рабочем столе и Android.
   * Сама панель от этого одна и та же во всех трёх оболочках.
   */
  device: DeviceSettingsClient;
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
  /**
   * Клиент открытого vault (фича 006). Нужен разделу «Vault», чтобы показать и
   * менять настройки, принадлежащие **vault**, а не устройству: политику
   * расположения файлов вставок и список помеченных разделов (FR-002, FR-015).
   *
   * Инъецируется хозяином панели по той же причине, что `confirmSwitch` и
   * `saveNote`: панель не владеет vault-бэкендом и не должна начать им владеть
   * ради одной настройки. `null` — vault не открыт, и выбирать нечего.
   */
  vaultClient?: VaultClient | null;
  /**
   * Vault переключён: ядро приняло смену `current_vault` (US2 фичи 004, AS3).
   *
   * Панель на этом и заканчивает: открыть выбранный vault в окне она не может
   * — клиентом владеет рабочая область, и переоткрытие делает она. Без этого
   * колбэка переключение меняло бы настройку, оставляя дерево на заметках
   * прежней папки.
   */
  onVaultSwitched?: (id: string) => void;
  /**
   * Клиент предпросмотра (фича 007): перечень установленных внешних шаблонов в
   * разделе «Vault». `null` — сборка ещё не готова, перечня нет.
   */
  preview?: PreviewClient | null;
}

export function SettingsPanel({
  device,
  confirmSwitch,
  saveNote,
  fullBackendUnavailableReason = null,
  vaultClient = null,
  onVaultSwitched,
  preview = null,
}: SettingsPanelProps): React.JSX.Element {
  const {
    state,
    apply,
    resetToDefaults,
    makeSettingsDefault,
    refresh,
    dismissNotice,
  } = useSettings(device);
  const vaults = useVaultList(device, {
    onChanged: refresh,
    ...(confirmSwitch ? { confirmSwitch } : {}),
    ...(onVaultSwitched ? { onSwitched: onVaultSwitched } : {}),
  });
  const vaultSettings = useVaultSettings(vaultClient);
  // Сочетания клавиш живут в настройках vault, а не устройства: см.
  // шорткаты едут вместе с папкой заметок, а не с устройством.
  const shortcuts = useShortcuts(vaultClient, vaultSettings.overrides);
  // Открытый раздел. «Vault» по умолчанию — первый пункт макета и то, с чего
  // начинается работа: без выбранной папки остальные разделы не о чем.
  const [section, setSection] = useState<SettingsSection>("vault");
  const [modeMessage, setModeMessage] = useState<string | null>(null);
  const [resetMessage, setResetMessage] = useState<string | null>(null);
  const [defaultsMessage, setDefaultsMessage] = useState<string | null>(null);

  /**
   * Направляет изменение в то хранилище, которое назвало ядро (фича 006,
   * FR-029).
   *
   * Ветка здесь — исполнение решения, а не само решение: панель не владеет
   * обоими хранилищами и не имеет права выбирать за ядро, куда пишется правка.
   * Та же форма, что у `ApplyPlan`: ядро говорит, интерфейс делает.
   */
  const route = async (change: SettingsChange): Promise<void> => {
    const vaultIsOpen = vaultSettings.overrides !== null;
    const target = await device.changeTarget(change, vaultIsOpen);
    if (target.target === "Device") {
      await apply(change);
      return;
    }
    await vaultSettings.applyChange(change);
  };

  /**
   * Сделать действующие значения умолчаниями устройства (FR-031). Отказ
   * показывается словами по той же причине, что и у сброса.
   */
  const runMakeDefault = async (): Promise<void> => {
    if (!vaultSettings.overrides) return;
    const plan = await makeSettingsDefault(vaultSettings.overrides);
    if (!plan) return;
    setDefaultsMessage(
      plan.outcome === "Applied"
        ? "Текущие значения стали умолчаниями. Папки без своих настроек будут открываться с ними."
        : describeResetRejection(plan),
    );
  };

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

  // Показываются и применяются **действующие** значения: устройство как слой
  // по умолчанию, vault перекрывает записанное в нём (FR-030). Иначе тема или
  // размер, ушедшие в vault, не отобразились бы в самой панели, которая их и
  // поменяла, — и не доехали бы до редактора.
  //
  // Слои складывает ядро, и за его границей это асинхронный вопрос. Пока ответ
  // не пришёл, показывается прежний — иначе при каждой правке тема мигала бы
  // значением устройства.
  const [resolved, setResolved] = useState<Settings | null>(null);
  const overrides = vaultSettings.overrides;
  useEffect(() => {
    if (!state.settings || !overrides) {
      setResolved(null);
      return;
    }
    let cancelled = false;
    device.resolveWithVault(overrides).then(
      (next) => {
        if (!cancelled) setResolved(next);
      },
      () => {
        // Не сложилось — остаются значения устройства, панель не падает.
      },
    );
    return () => {
      cancelled = true;
    };
  }, [device, state.settings, overrides]);
  const settings = overrides && resolved ? resolved : state.settings;

  // FR-017, SC-002: тема и размер шрифта доходят до остального интерфейса через
  // CSS-переменные на корне документа — в том числе когда значения изменились
  // снаружи, а не через панель.
  //
  // Второе место, ставящее ту же переменную, — `Workspace`: он применяет
  // перекрытие vault во всех трёх оболочках. Разойтись они не могут: там, где
  // перекрытие задано, оно же и есть действующее значение.
  useAppearance(settings);

  if (state.isLoading || !settings) {
    return <p role="status">Загружаем настройки…</p>;
  }

  // Подвал меню: версия — из ядра, имя папки — у клиента открытого vault.
  // Ни то, ни другое не пишется здесь литералом: в макете стоят «0.9.2» и
  // «диссертация-2026», но это подписи макета, а не факты приложения.
  const version = state.diagnostics?.app_version ?? "";
  const vaultName = vaultClient?.rootName ?? null;

  return (
    <div className="ts-settings" aria-label="Настройки">
      {/* Левое меню разделов (макет «Typsidian Утилиты» § Настройки). Разделы
          переключаются, а не идут стопкой: их четыре, и «Кейбиндинги» в конце
          длинного свитка пользователь просто не найдёт. */}
      <nav className="ts-settings__nav" aria-label="Разделы настроек">
        <span className="ts-eyebrow">Настройки</span>
        {SECTIONS.map((item) => (
          <button
            key={item.id}
            type="button"
            className={
              item.id === section
                ? "ts-settings__navitem ts-settings__navitem--active"
                : "ts-settings__navitem"
            }
            aria-current={item.id === section ? "page" : undefined}
            onClick={() => setSection(item.id)}
          >
            {item.title}
          </button>
        ))}
        <span className="ts-spacer" />
        <div className="ts-settings__foot">
          typsidian {version}
          <br />
          {vaultName === null ? "vault не открыт" : `vault: ${vaultName}`}
        </div>
      </nav>

      <div className="ts-settings__body">
        {/* Уведомления о состоянии хранилища видны в любом разделе: они про
            весь файл настроек, а не про раздел, в котором пользователь сейчас
            стоит. */}
        {state.notices.map((notice) => (
          <StorageNoticeBanner
            key={notice.kind}
            notice={notice}
            onDismiss={() => dismissNotice(notice.kind)}
          />
        ))}

        {section === "appearance" && (
          <AppearanceSection
            settings={settings}
            onChangeTheme={(theme) => void route(changeOf({ theme }))}
            onChangeFontSize={(size) =>
              void route(changeOf({ editor_font_size: size }))
            }
            canMakeDefault={vaultSettings.overrides !== null}
            onMakeDefault={() => void runMakeDefault()}
            defaultsMessage={defaultsMessage}
            rejectionMessage={describeRejection(state)}
          />
        )}

        {section === "vault" && (
          <VaultSection
            knownVaults={state.knownVaults}
            currentVaultId={settings.current_vault ?? null}
            onAddFolder={vaults.addFolder}
            onSwitch={vaults.switchTo}
            onSetLabel={vaults.setLabel}
            onForget={vaults.forget}
            errorMessage={
              vaultSettings.error ??
              (vaults.error ? describeVaultActionError(vaults.error) : null)
            }
            assetsPlacement={
              vaultSettings.overrides
                ? (vaultSettings.overrides.assets_placement ?? "next-to-note")
                : null
            }
            onChangeAssetsPlacement={vaultSettings.setAssetsPlacement}
            sections={(vaultSettings.overrides?.sections ?? []).map(
              (marked) => marked.path,
            )}
          />
        )}

        {section === "templates" && (
          // Отдельный пункт, как в панели папки desktop/Android (007 FR-016):
          // внизу раздела «Vault» его не находили.
          <section aria-label="Шаблоны">
            <div className="ts-settings__head">
              <h2>Шаблоны</h2>
              <span className="ts-settings__meta">
                {vaultName === null ? "vault не открыт" : `настройки папки · ${vaultName}`}
              </span>
            </div>
            <TemplatesSection vaultSettings={vaultSettings} preview={preview} />
          </section>
        )}

        {section === "keybindings" && (
          <>
            <KeybindingsSection
              mode={state.editorMode}
              configDiskState={state.configDiskState}
              configPath={settings.editor_config_path ?? null}
              fullBackendUnavailableReason={fullBackendUnavailableReason}
              onChooseFullBackend={() => void changeMode("fullBackend")}
              onChooseSimplifiedEmulation={() =>
                void changeMode("simplifiedEmulation")
              }
              onPickConfig={() => {}}
              onApplyConfig={() => {}}
              onLeavePlainInput={() => {}}
            />

            {/* FR-029: о вынужденном сохранении заметки и об отказе сменить
                режим пользователь узнаёт словами, а не по молчаливому
                бездействию. */}
            {modeMessage && <p role="status">{modeMessage}</p>}

            {/* Сочетания команд приложения — рядом с режимом редактирования, но
                это разные вещи: выше настраивается, чем обрабатываются клавиши
                внутри редактора, здесь — какими клавишами вызываются команды
                самого приложения. Хранятся они в настройках vault, поэтому и
                вызовы идут через `vaultSettings`, а не через `route`. */}
            <ShortcutsSection
              bindings={shortcuts.bindings}
              error={vaultSettings.error}
              onAssign={(command, shortcut) =>
                void vaultSettings.setKeybinding(command, shortcut)
              }
              onClear={(command) => void vaultSettings.setKeybinding(command, null)}
              onReset={(command) => void vaultSettings.resetKeybinding(command)}
            />
          </>
        )}

        {section === "advanced" && (
          <AdvancedSection
            capabilities={state.capabilities}
            diagnostics={state.diagnostics}
            onReveal={() => void device.reveal().catch(() => {})}
            onResetToDefaults={() => void runReset()}
            resetMessage={resetMessage}
          />
        )}
      </div>
    </div>
  );
}

/**
 * Разделы левого меню в порядке макета.
 *
 * Порядок именно такой и значим: «Vault» первым, потому что без открытой папки
 * остальное не о чем; «Дополнительно» последним, потому что там сброс и
 * диагностика.
 */
const SECTIONS: { id: SettingsSection; title: string }[] = [
  { id: "vault", title: "Vault" },
  { id: "templates", title: "Шаблоны" },
  { id: "appearance", title: "Внешний вид" },
  { id: "keybindings", title: "Кейбиндинги" },
  { id: "advanced", title: "Дополнительно" },
];

/** Какой раздел показан. */
export type SettingsSection =
  | "vault"
  | "templates"
  | "appearance"
  | "keybindings"
  | "advanced";

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
