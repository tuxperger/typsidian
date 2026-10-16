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
import { SyncSettingsSection } from "../sync/SyncSettingsSection.js";
import type { SyncControl } from "../sync/useSync.js";
import { useVaultSettings } from "./useVaultSettings.js";
import type { VaultClient } from "../vault-client.js";
import type { PreviewClient } from "../preview-client.js";
import { TemplatesSection } from "./TemplatesSection.js";
import { CaptureSection } from "./CaptureSection.js";
import { useShortcuts } from "../workspace/useShortcuts.js";
import { useI18n, type MessageKey, type Phrase, type Translate } from "../i18n/index.js";

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
  /**
   * Синхронизация открытого vault (фича 011): её раздел настроек. `null` —
   * vault не открыт или оболочка синхронизацию не дала.
   */
  sync?: SyncControl | null;
  /** Жест «создать vault из удалённого репозитория» (011 FR-013). */
  onCloneVault?: (() => void) | null;
}

export function SettingsPanel({
  device,
  confirmSwitch,
  saveNote,
  fullBackendUnavailableReason = null,
  vaultClient = null,
  onVaultSwitched,
  preview = null,
  sync = null,
  onCloneVault = null,
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
  const { t } = useI18n();
  // Итоги действий — фразами, а не строками: показанное сообщение обязано
  // перерисоваться на новом языке (фича 010, FR-014).
  const [modeMessage, setModeMessage] = useState<Phrase | null>(null);
  const [resetMessage, setResetMessage] = useState<Phrase | null>(null);
  const [defaultsMessage, setDefaultsMessage] = useState<Phrase | null>(null);

  /**
   * Направляет изменение в те хранилища, которые назвало ядро (фича 006,
   * FR-029; фича 010, FR-016).
   *
   * Ветка здесь — исполнение решения, а не само решение: панель не владеет
   * обоими хранилищами и не имеет права выбирать за ядро, куда пишется правка.
   * Набор может распасться на несколько записей (язык всегда в устройство,
   * тема — в открытый vault), и они исполняются по порядку.
   */
  const route = async (change: SettingsChange): Promise<void> => {
    const vaultIsOpen = vaultSettings.overrides !== null;
    for (const routed of await device.routeChange(change, vaultIsOpen)) {
      if (routed.target.target === "Device") {
        await apply(routed.change);
      } else {
        await vaultSettings.applyChange(routed.change);
      }
    }
  };

  /**
   * Сделать действующие значения умолчаниями устройства (FR-031). Отказ
   * показывается словами по той же причине, что и у сброса.
   */
  const runMakeDefault = async (): Promise<void> => {
    if (!vaultSettings.overrides) return;
    const plan = await makeSettingsDefault(vaultSettings.overrides);
    if (!plan) return;
    setDefaultsMessage(() =>
      plan.outcome === "Applied"
        ? (tr: Translate) => tr("settings.defaults.applied")
        : (tr: Translate) => describeResetRejection(tr, plan),
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
    setResetMessage(() =>
      plan.outcome === "Applied"
        ? (tr: Translate) => tr("settings.reset.applied")
        : (tr: Translate) => describeResetRejection(tr, plan),
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
          setModeMessage(() => (tr: Translate) => tr("settings.mode.saveFirst"));
          return false;
        }),
    });
    if (outcome) {
      const message = describeApplyOutcome(outcome);
      setModeMessage(() => message);
    }
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

  if (state.isLoading) {
    return <p role="status">{t("settings.loading")}</p>;
  }
  if (!settings) {
    // Снимок не пришёл вовсе — показывать нечего, кроме причины (010 FR-029).
    return <p role="alert">{t("settings.notice.unreadable")}</p>;
  }

  // Подвал меню: версия — из ядра, имя папки — у клиента открытого vault.
  // Ни то, ни другое не пишется здесь литералом: в макете стоят «0.9.2» и
  // «диссертация-2026», но это подписи макета, а не факты приложения.
  const version = state.diagnostics?.app_version ?? "";
  const vaultName = vaultClient?.rootName ?? null;

  return (
    <div className="ts-settings" aria-label={t("workspace.settings")}>
      {/* Левое меню разделов (макет «Typsidian Утилиты» § Настройки). Разделы
          переключаются, а не идут стопкой: их четыре, и «Кейбиндинги» в конце
          длинного свитка пользователь просто не найдёт. */}
      <nav className="ts-settings__nav" aria-label={t("settings.sections")}>
        <span className="ts-eyebrow">{t("workspace.settings")}</span>
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
            {t(item.title)}
          </button>
        ))}
        <span className="ts-spacer" />
        <div className="ts-settings__foot">
          typsidian {version}
          <br />
          {vaultName === null
            ? t("settings.vaultNotOpen")
            : t("settings.vaultName", { name: vaultName })}
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
            onChangeLanguage={(language) => void route(changeOf({ language }))}
            onChangeFontSize={(size) =>
              void route(changeOf({ editor_font_size: size }))
            }
            canMakeDefault={vaultSettings.overrides !== null}
            onMakeDefault={() => void runMakeDefault()}
            defaultsMessage={defaultsMessage === null ? null : defaultsMessage(t)}
            rejectionMessage={describeRejection(t, state)}
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
              vaultSettings.error?.(t) ??
              (vaults.error ? describeVaultActionError(t, vaults.error) : null)
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
            exportFolder={
              vaultSettings.overrides
                ? (vaultSettings.overrides.export_folder ?? "export")
                : null
            }
            onChangeExportFolder={vaultSettings.setExportFolder}
            onCloneVault={onCloneVault}
          />
        )}

        {section === "sync" && (
          <section aria-label={t("sync.settings.title")}>
            <h2>{t("sync.settings.title")}</h2>
            {sync === null ? (
              <p role="status">{t("settings.vaultNotOpen")}</p>
            ) : (
              <SyncSettingsSection sync={sync} />
            )}
          </section>
        )}

        {section === "templates" && (
          // Отдельный пункт, как в панели папки desktop/Android (007 FR-016):
          // внизу раздела «Vault» его не находили.
          <section aria-label={t("settings.section.templates")}>
            <div className="ts-settings__head">
              <h2>{t("settings.section.templates")}</h2>
              <span className="ts-settings__meta">
                {vaultName === null
                  ? t("settings.vaultNotOpen")
                  : t("settings.folderSettings", { name: vaultName })}
              </span>
            </div>
            <TemplatesSection vaultSettings={vaultSettings} preview={preview} />
          </section>
        )}

        {section === "capture" && (
          <section aria-label={t("capture.settings.title")}>
            <div className="ts-settings__head">
              <h2>{t("capture.settings.title")}</h2>
              <span className="ts-settings__meta">
                {vaultName === null
                  ? t("settings.vaultNotOpen")
                  : t("settings.folderSettings", { name: vaultName })}
              </span>
            </div>
            <CaptureSection vaultSettings={vaultSettings} client={vaultClient} />
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
            {modeMessage && <p role="status">{modeMessage(t)}</p>}

            {/* Сочетания команд приложения — рядом с режимом редактирования, но
                это разные вещи: выше настраивается, чем обрабатываются клавиши
                внутри редактора, здесь — какими клавишами вызываются команды
                самого приложения. Хранятся они в настройках vault, поэтому и
                вызовы идут через `vaultSettings`, а не через `route`. */}
            <ShortcutsSection
              bindings={shortcuts.bindings}
              error={vaultSettings.error === null ? null : vaultSettings.error(t)}
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
            loadJournal={() => device.journal()}
            resetMessage={resetMessage === null ? null : resetMessage(t)}
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
const SECTIONS: { id: SettingsSection; title: MessageKey }[] = [
  { id: "vault", title: "settings.section.vault" },
  { id: "templates", title: "settings.section.templates" },
  { id: "capture", title: "capture.settings.title" },
  { id: "sync", title: "sync.panel.title" },
  { id: "appearance", title: "settings.section.appearance" },
  { id: "keybindings", title: "settings.section.keybindings" },
  { id: "advanced", title: "settings.section.advanced" },
];

/** Какой раздел показан. */
export type SettingsSection =
  | "vault"
  | "templates"
  | "capture"
  | "sync"
  | "appearance"
  | "keybindings"
  | "advanced";

/**
 * Причина отказа берётся из плана, вернувшегося из ядра (FR-016): UI
 * сопоставляет вариантам формулировки, но не решает сам, что допустимо.
 */
function describeRejection(t: Translate, state: SettingsState): string | null {
  const plan = state.lastPlan;
  if (!plan || plan.outcome !== "Rejected") return null;
  switch (plan.reason.kind) {
    case "ValueOutOfRange":
      return t("settings.reject.valueOutOfRange");
    case "SaveFailed":
      return t("settings.reject.saveFailed");
    case "UnresolvedConflict":
      return t("settings.reject.unresolvedConflict");
    case "StorageNotWritable":
      return t("settings.reject.storageNotWritable");
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
function describeResetRejection(t: Translate, plan: ApplyPlan): string {
  if (plan.outcome !== "Rejected") return t("settings.reset.notDone");
  switch (plan.reason.kind) {
    case "UnresolvedConflict":
      return t("settings.reset.unresolvedConflict");
    case "SaveFailed":
      return t("settings.reset.saveFailed");
    case "StorageNotWritable":
      return t("settings.reset.storageNotWritable");
    case "ValueOutOfRange":
      // Недостижимо: набор собирает ядро из своих же умолчаний. Ветка
      // существует ради исчерпывающей проверки (принцип IV).
      return t("settings.reset.valueOutOfRange");
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
  const { t } = useI18n();
  return (
    <div role="status">
      <p>{describeNotice(t, notice)}</p>
      <button type="button" onClick={onDismiss}>
        {t("common.ok")}
      </button>
    </div>
  );
}

function describeNotice(t: Translate, notice: StorageNotice): string {
  switch (notice.kind) {
    case "unreadable":
      // FR-013: подчёркиваем, что файл пользователя не тронут — иначе
      // сообщение читается как «мы его починили за вас».
      return t("settings.notice.unreadable");
    case "readOnly":
      return t("settings.notice.readOnly");
    case "invalidSetAside":
      return t("settings.notice.invalidSetAside", { path: notice.movedTo ?? "—" });
    default: {
      const exhaustive: never = notice.kind;
      return exhaustive;
    }
  }
}
