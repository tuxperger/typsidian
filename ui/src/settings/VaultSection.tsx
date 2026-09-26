import { useState } from "react";
import type {
  AssetsPlacement,
  VaultRecord,
} from "../wasm-core/typsidian_core.js";

/** Варианты расположения файлов вставок (фича 006, FR-001). */
const PLACEMENTS: ReadonlyArray<{
  value: AssetsPlacement;
  title: string;
  hint: string;
}> = [
  {
    value: "next-to-note",
    title: "Рядом с заметкой",
    hint: "Одна общая папка assets в той же папке, где лежит заметка.",
  },
  {
    value: "vault-root",
    title: "В корне vault",
    hint: "Одна папка assets на весь vault, рядом с заметками ничего не создаётся.",
  },
  {
    value: "section-root",
    title: "В корне раздела",
    hint: "В ближайшей папке-предке, помеченной как раздел. Если помеченных нет — в корне vault.",
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
}: VaultSectionProps): React.JSX.Element {
  const [labelDraft, setLabelDraft] = useState<{ id: string; value: string } | null>(null);
  const current = knownVaults.find((record) => record.id === currentVaultId) ?? null;

  return (
    <section aria-label="Vault">
      <h2>Vault</h2>

      <p>
        Текущий vault:{" "}
        {current ? (
          // Показывается подпись, если задана, иначе расположение (FR-018).
          // Само расположение остаётся доступным рядом.
          <>
            <strong>{displayName(current)}</strong>
            {current.label && <span> ({current.location})</span>}
          </>
        ) : (
          <em>не выбран</em>
        )}
      </p>

      <button type="button" onClick={() => void onAddFolder()}>
        Добавить папку
      </button>

      {errorMessage && <p role="alert">{errorMessage}</p>}

      <ul aria-label="Известные vault">
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
                <span role="note"> — недоступен, добавьте папку заново</span>
              )}

              {isEditingLabel ? (
                <>
                  <input
                    type="text"
                    autoFocus
                    aria-label={`Подпись для ${record.location}`}
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
                    Отмена
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  onClick={() => setLabelDraft({ id: record.id, value: record.label ?? "" })}
                >
                  {record.label ? "Изменить подпись" : "Подписать"}
                </button>
              )}

              {!isCurrent && !isUnavailable && (
                <button type="button" onClick={() => void onSwitch(record.id)}>
                  Переключиться
                </button>
              )}

              <button type="button" onClick={() => void onForget(record.id)}>
                Убрать из списка
              </button>
            </li>
          );
        })}
      </ul>

      {knownVaults.length === 0 && (
        <p role="status">Пока ни одной папки не добавлено.</p>
      )}

      {assetsPlacement !== null && onChangeAssetsPlacement && (
        <fieldset className="ts-assets-placement">
          <legend>Файлы вставленных досок</legend>
          <p className="ts-assets-placement__intro">
            Где создаётся папка <code>assets</code>. Настройка принадлежит этому
            vault и переезжает вместе с ним. Уже созданные файлы смена варианта
            не двигает.
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
              <span className="ts-assets-placement__title">{option.title}</span>
              <span className="ts-assets-placement__hint">{option.hint}</span>
            </label>
          ))}

          {assetsPlacement === "section-root" && (
            <p role="status" className="ts-assets-placement__sections">
              {sections.length > 0 ? (
                <>Помеченные разделы: {sections.join(", ")}.</>
              ) : (
                <>
                  Помеченных разделов пока нет — вставки идут в корень vault.
                  Пометить папку разделом можно из дерева.
                </>
              )}
            </p>
          )}
        </fieldset>
      )}
    </section>
  );
}

/** Подпись, если задана, иначе расположение (FR-018). */
function displayName(record: VaultRecord): string {
  const label = record.label?.trim();
  return label && label.length > 0 ? label : record.location;
}
