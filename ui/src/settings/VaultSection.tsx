import { useState } from "react";
import { useI18n, type MessageKey } from "../i18n/index.js";
import type {
  AssetsPlacement,
  VaultRecord,
} from "../wasm-core/typsidian_core.js";

/** Варианты расположения файлов вставок (фича 006, FR-001). */
const PLACEMENTS: ReadonlyArray<{
  value: AssetsPlacement;
  title: MessageKey;
  hint: MessageKey;
}> = [
  {
    value: "next-to-note",
    title: "settings.assets.nextToNote",
    hint: "settings.assets.nextToNoteHint",
  },
  {
    value: "vault-root",
    title: "settings.assets.vaultRoot",
    hint: "settings.assets.vaultRootHint",
  },
  {
    value: "section-root",
    title: "settings.assets.sectionRoot",
    hint: "settings.assets.sectionRootHint",
  },
];

export interface VaultSectionProps {
  knownVaults: VaultRecord[];
  currentVaultId: string | null;
  /** Выбор папки — жест пользователя, выполняется вызывающей стороной (T064). */
  onAddFolder: () => void | Promise<void>;
  onSwitch: (id: string) => void | Promise<void>;
  onSetLabel: (id: string, label: string | null) => void | Promise<void>;
  onForget: (id: string) => void | Promise<void>;
  /** Создать vault из удалённого репозитория (011 FR-013); `null` — оболочка жеста не даёт. */
  onCloneVault?: (() => void) | null;
  /** Причина отказа последней операции — показывается, но не придумывается. */
  errorMessage?: string | null;

  /**
   * Текущее расположение файлов вставленных досок (фича 006, FR-001).
   *
   * `null` — vault не открыт: выбирать нечего, потому что настройка
   * принадлежит vault, а не устройству (FR-002).
   */
  assetsPlacement?: AssetsPlacement | null;
  onChangeAssetsPlacement?: (placement: AssetsPlacement) => void | Promise<void>;
  /** Помеченные разделы — показываются, чтобы выбор «в корне раздела» был осмыслен. */
  sections?: string[];

  /**
   * Папка экспорта в PDF (фича 009, FR-028) — уже с умолчанием: `export`,
   * если не настроена. `null` — vault не открыт.
   *
   * Умолчание подставляет вызывающая сторона, а не этот компонент: «не
   * настроено» в перекрытиях — `undefined`, а параметр по умолчанию
   * деструктуризации превращал бы его в `null`, то есть в «vault не открыт».
   */
  exportFolder?: string | null;
  /** Записать папку; `false` — ядро отклонило значение (FR-029). */
  onChangeExportFolder?: (folder: string | null) => Promise<boolean>;
}

/**
 * Раздел «Vault» (T063, FR-018…FR-024).
 *
 * Показывает расположение текущего vault и список известных с пометкой
 * недоступных, позволяет добавить папку, подписать запись, переключиться и
 * убрать запись из списка.
 *
 * Чего этот компонент не делает никогда: **не сравнивает папки сам** и не
 * судит, та ли это папка. Он передаёт полученную ссылку в ядро и доверяет
 * ответу (FR-020, принцип I). Две записи, выглядящие одинаково, — не ошибка
 * отрисовки, а две разные папки с одинаковым именем; различать их пользователю
 * помогает подпись (research.md §14), а не догадки UI.
 */
export function VaultSection({
  knownVaults,
  currentVaultId,
  onAddFolder,
  onSwitch,
  onSetLabel,
  onForget,
  errorMessage = null,
  assetsPlacement = null,
  onChangeAssetsPlacement,
  sections = [],
  exportFolder = null,
  onChangeExportFolder,
  onCloneVault = null,
}: VaultSectionProps): React.JSX.Element {
  const { t } = useI18n();
  // `%` в строке каталога — место кода: слова вокруг него расставляет язык.
  const [assetsBefore = "", assetsAfter = ""] = t("settings.assets.intro").split("%");
  const [labelDraft, setLabelDraft] = useState<{ id: string; value: string } | null>(null);
  // Черновик поля папки экспорта: пишется по Enter или уходу из поля, а не на
  // каждое нажатие — промежуточное «Выгру» не должно становиться настройкой.
  const [folderDraft, setFolderDraft] = useState<string | null>(null);
  const current = knownVaults.find((record) => record.id === currentVaultId) ?? null;

  return (
    <section aria-label={t("settings.section.vault")}>
      <h2>{t("settings.section.vault")}</h2>

      <p>
        {t("settings.vaults.current")}{" "}
        {current ? (
          // Показывается подпись, если задана, иначе расположение (FR-018).
          // Само расположение остаётся доступным рядом.
          <>
            <strong>{displayName(current)}</strong>
            {current.label && <span> ({current.location})</span>}
          </>
        ) : (
          <em>{t("settings.vaults.noneSelected")}</em>
        )}
      </p>

      <button type="button" onClick={() => void onAddFolder()}>
        {t("settings.vaults.add")}
      </button>
      {onCloneVault !== null && (
        <button type="button" onClick={onCloneVault}>
          {t("sync.command.clone")}
        </button>
      )}

      {errorMessage && <p role="alert">{errorMessage}</p>}

      <ul aria-label={t("settings.vaults.known")}>
        {knownVaults.map((record) => {
          const isCurrent = record.id === currentVaultId;
          const isUnavailable = record.availability === "unavailable";
          const isEditingLabel = labelDraft?.id === record.id;

          return (
            <li key={record.id} aria-current={isCurrent ? "true" : undefined}>
              <span>{displayName(record)}</span>
              {isUnavailable && (
                // FR-023: связь с папкой утрачена. Приложение её не
                // разыскивает — запись добавляется заново.
                <span role="note">{t("settings.vaults.unavailable")}</span>
              )}

              {isEditingLabel ? (
                <>
                  <input
                    type="text"
                    autoFocus
                    aria-label={t("settings.vaults.labelFor", { location: record.location })}
                    value={labelDraft.value}
                    onChange={(event) =>
                      setLabelDraft({ id: record.id, value: event.target.value })
                    }
                    onKeyDown={(event) => {
                      if (event.key === "Escape") setLabelDraft(null);
                      if (event.key === "Enter") {
                        event.preventDefault();
                        // Пустая строка — снятие подписи: уникальности она не
                        // требует и в тождестве папок не участвует.
                        void onSetLabel(record.id, labelDraft.value.trim() || null);
                        setLabelDraft(null);
                      }
                    }}
                  />
                  <button type="button" onClick={() => setLabelDraft(null)}>
                    {t("common.cancel")}
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  onClick={() => setLabelDraft({ id: record.id, value: record.label ?? "" })}
                >
                  {record.label ? t("settings.vaults.editLabel") : t("settings.vaults.addLabel")}
                </button>
              )}

              {!isCurrent && !isUnavailable && (
                <button type="button" onClick={() => void onSwitch(record.id)}>
                  {t("settings.vaults.switch")}
                </button>
              )}

              <button type="button" onClick={() => void onForget(record.id)}>
                {t("settings.vaults.forget")}
              </button>
            </li>
          );
        })}
      </ul>

      {knownVaults.length === 0 && (
        <p role="status">{t("settings.vaults.empty")}</p>
      )}

      {assetsPlacement !== null && onChangeAssetsPlacement && (
        <fieldset className="ts-assets-placement">
          <legend>{t("settings.assets.legend")}</legend>
          <p className="ts-assets-placement__intro">
            {assetsBefore}
            <code>assets</code>
            {assetsAfter}
          </p>

          {PLACEMENTS.map((option) => (
            <label key={option.value} className="ts-assets-placement__option">
              <input
                type="radio"
                name="assets-placement"
                value={option.value}
                checked={assetsPlacement === option.value}
                onChange={() => void onChangeAssetsPlacement(option.value)}
              />
              <span className="ts-assets-placement__title">{t(option.title)}</span>
              <span className="ts-assets-placement__hint">{t(option.hint)}</span>
            </label>
          ))}

          {assetsPlacement === "section-root" && (
            <p role="status" className="ts-assets-placement__sections">
              {sections.length > 0 ? (
                <>{t("settings.assets.sections", { list: sections.join(", ") })}</>
              ) : (
                <>{t("settings.assets.noSections")}</>
              )}
            </p>
          )}
        </fieldset>
      )}

      {exportFolder !== null && onChangeExportFolder && (
        <fieldset className="ts-assets-placement">
          <legend>{t("workspace.exportPdf")}</legend>
          <p className="ts-assets-placement__intro">
            {t("settings.export.intro")} <code>{t("settings.export.exampleNote")}.typ</code> →{" "}
            <code>{`${exportFolder}/${t("settings.export.exampleNote")}.pdf`}</code>.{" "}
            {t("settings.export.owned")}
          </p>
          <label className="ts-assets-placement__option">
            <span className="ts-assets-placement__title">{t("settings.export.folder")}</span>
            <input
              type="text"
              value={folderDraft ?? exportFolder}
              aria-label={t("settings.export.folder")}
              spellCheck={false}
              onChange={(event) => setFolderDraft(event.target.value)}
              onBlur={() => void commitFolder()}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void commitFolder();
                }
                if (event.key === "Escape") setFolderDraft(null);
              }}
            />
          </label>
        </fieldset>
      )}
    </section>
  );

  async function commitFolder(): Promise<void> {
    if (folderDraft === null || onChangeExportFolder === undefined) return;
    const value = folderDraft.trim();
    // Пустое поле и «export» — это умолчание: ключ уходит из файла.
    const ok = await onChangeExportFolder(value === "" || value === "export" ? null : value);
    // Отказ показывает раздел сообщением ядра; черновик остаётся в поле,
    // чтобы его можно было поправить, а не набирать заново.
    if (ok) setFolderDraft(null);
  }
}

/** Подпись, если задана, иначе расположение (FR-018). */
function displayName(record: VaultRecord): string {
  const label = record.label?.trim();
  return label && label.length > 0 ? label : record.location;
}
