"use client";

import { useEffect, useState } from "react";
import type { VaultClient } from "../vault-client.js";

// Две версии доски для диалога конфликта (T033).
//
// Обе берутся из ядра, ни одна не восстанавливается по памяти UI: буфер
// пользователя ядро держит само (`ConflictRegistry.local_buffer`, 001 FR-035), а
// внешняя версия читается с диска тем же `readBoard`, что и при открытии. Копия
// буфера в этом хуке была бы вторым источником истины ровно там, где 001 его
// устранил.

export interface BoardConflictVersions {
  /** Буфер пользователя из ядра; `null` — конфликта нет или буфер уже снят. */
  localContent: string | null;
  /** Версия на диске; `null` — прочитать не удалось (FR-016). */
  externalContent: string | null;
}

const NO_VERSIONS: BoardConflictVersions = {
  localContent: null,
  externalContent: null,
};

/**
 * Содержимое обеих версий конфликтующей доски.
 *
 * `boardPath === null` — конфликта по доске нет, и читать нечего.
 */
export function useBoardConflict(
  client: VaultClient | null,
  boardPath: string | null,
): BoardConflictVersions {
  const [versions, setVersions] = useState<BoardConflictVersions>(NO_VERSIONS);

  useEffect(() => {
    if (!client || boardPath === null) {
      setVersions(NO_VERSIONS);
      return;
    }
    let cancelled = false;
    setVersions(NO_VERSIONS);

    void (async () => {
      const conflict = await client.pendingConflict(boardPath);
      const localContent = conflict?.local_buffer ?? null;
      if (!cancelled) setVersions({ localContent, externalContent: null });

      // Отказ чтения — не исключение, а отсутствие превью с причиной: выбор
      // версии обязан остаться доступным (FR-016).
      let externalContent: string | null = null;
      try {
        externalContent = await client.readBoard(boardPath);
      } catch {
        externalContent = null;
      }
      if (!cancelled) setVersions({ localContent, externalContent });
    })();

    return () => {
      cancelled = true;
    };
  }, [client, boardPath]);

  return versions;
}
