"use client";

import type { Embed } from "../wasm-core/typsidian_core.js";

// Индикатор актуальности вставки (T057, T058, T059, FR-027, FR-031, FR-032,
// SC-008).
//
// Пять состояний показываются пятью разными способами. Схлопывать их нельзя, и
// это не вкусовщина: `Undeterminable`, слитый с `UpToDate`, даёт молчаливое
// расхождение, которое спека запрещает прямо (SC-008), а `SnapshotMissing`,
// слитый с `BoardMissing`, отнимает у пользователя действие восстановления
// (FR-032 против FR-031).
//
// Исчерпывающий разбор объединения обеспечивает TypeScript: `never` в `default`
// не даст добавить шестое состояние в ядре и забыть его здесь (принцип IV).

export interface EmbedStaleBadgeProps {
  embed: Embed;
  /** Идёт запись снимка по явному действию пользователя. */
  isUpdating?: boolean;
  /** Причина отказа словами; `null` — отказа не было. */
  error?: string | null;
  /** Привести снимок в соответствие с доской (FR-028). */
  onUpdate: (embed: Embed) => void;
  /** Восстановить отсутствующий снимок из доски (FR-032). */
  onRestore: (embed: Embed) => void;
}

export function EmbedStaleBadge({
  embed,
  isUpdating = false,
  error = null,
  onUpdate,
  onRestore,
}: EmbedStaleBadgeProps): React.JSX.Element | null {
  const state = embed.state;
  // Перестроить снимок можно только из известной доски. Адрес доски живёт в
  // метаданных снимка (research.md §5), поэтому там, где метаданных нет, нет и
  // источника: предлагать действие, которое неоткуда выполнить, было бы обманом.
  const source = embed.board_path;

  switch (state.state) {
    case "UpToDate":
      // data-model.md: пользователь не видит ничего дополнительного. Вставка
      // соответствует доске — сообщать об этом нечего.
      return error === null ? null : <Failure error={error} />;

    case "Stale":
      return (
        <span className="ts-embed ts-embed--stale">
          <span role="status">Вставка устарела</span>
          <button type="button" disabled={isUpdating} onClick={() => onUpdate(embed)}>
            Обновить вставку
          </button>
          {error !== null && <Failure error={error} />}
        </span>
      );

    case "SnapshotMissing":
      return (
        <span className="ts-embed ts-embed--stale">
          <span role="status">
            Снимок отсутствует
            {source === undefined && ", а доска-источник в нём не записана"}
          </span>
          {source !== undefined && (
            <button type="button" disabled={isUpdating} onClick={() => onRestore(embed)}>
              Восстановить снимок
            </button>
          )}
          {error !== null && <Failure error={error} />}
        </span>
      );

    case "BoardMissing":
      // FR-031: заметка остаётся читаемой и редактируемой — здесь нет ни
      // блокировки, ни предложения что-то починить за пользователя.
      return (
        <span className="ts-embed ts-embed--missing">
          <span role="status">
            Источник вставки отсутствует: доску удалили или переместили
          </span>
          {error !== null && <Failure error={error} />}
        </span>
      );

    case "Undeterminable":
      return (
        <span className="ts-embed ts-embed--unknown">
          <span role="status">Сравнить не удалось: {state.reason}</span>
          {source !== undefined && (
            <button type="button" disabled={isUpdating} onClick={() => onUpdate(embed)}>
              Перестроить снимок
            </button>
          )}
          {error !== null && <Failure error={error} />}
        </span>
      );

    default: {
      const exhaustive: never = state;
      return exhaustive;
    }
  }
}

function Failure({ error }: { error: string }): React.JSX.Element {
  return (
    <span className="ts-eyebrow ts-eyebrow--alert" role="alert">
      {error}
    </span>
  );
}
