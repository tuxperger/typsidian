"use client";

import type { Embed } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";

// Переход от вставки в тексте к её доске на полотне (T047, FR-025).
//
// Адрес доски приходит из ядра, из метаданных снимка (research.md §5), а не
// выводится из имени файла снимка: имя вставки к доске отношения не имеет.
// Поэтому переход возможен не всегда — и когда он невозможен, это сказано, а не
// подменено догадкой (`Embed.board_path === undefined`).

export interface EmbedLinkProps {
  embed: Embed;
  onOpenBoard: (boardPath: string) => void;
}

export function EmbedLink({ embed, onOpenBoard }: EmbedLinkProps): React.JSX.Element {
  const { t } = useI18n();
  const boardPath = embed.board_path;

  if (boardPath === undefined) {
    return (
      <span className="ts-eyebrow">
        {t("boards.embed.unknownSource", { id: embed.id.value })}
      </span>
    );
  }

  return (
    <button
      type="button"
      className="ts-btn"
      onClick={() => onOpenBoard(boardPath)}
      title={boardPath}
    >
      {t("boards.embed.open", { name: boardName(boardPath) })}
    </button>
  );
}

/** Имя файла доски: путь целиком в кнопке не читается. */
function boardName(boardPath: string): string {
  const lastSlash = boardPath.lastIndexOf("/");
  return lastSlash === -1 ? boardPath : boardPath.slice(lastSlash + 1);
}
