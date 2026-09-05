import { useState } from "react";
import type { VaultRecord } from "../wasm-core/typsidian_core.js";

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
    </section>
  );
}

/** Подпись, если задана, иначе расположение (FR-018). */
function displayName(record: VaultRecord): string {
  const label = record.label?.trim();
  return label && label.length > 0 ? label : record.location;
}
