"use client";

import { useEffect, useState } from "react";
import type { PreviewClient } from "../preview-client.js";
import type { LinkedIndex } from "../wasm-core/typsidian_core.js";

/**
 * Метки связанных файлов открытой заметки (фича 018, research.md §3).
 * Пересчёт — при смене заметки и на каждый новый список зависимостей сборки:
 * сборка запускается и от смены отметки зависимостей, так что правка
 * связанного файла доходит сюда тем же путём. Байты ядро берёт из кэша
 * сборки, если файл не менялся. Ответ про другую заметку отбрасывается.
 */
export function useLinkedIndex(
  client: PreviewClient | null,
  notePath: string | null,
  dependencies: readonly string[],
): LinkedIndex | null {
  const [index, setIndex] = useState<LinkedIndex | null>(null);

  useEffect(() => {
    if (client?.completionLinkedIndex === undefined || notePath === null) {
      setIndex(null);
      return;
    }
    let alive = true;
    void client
      .completionLinkedIndex(notePath, [...dependencies])
      .then((found) => {
        if (alive && found.note_path === notePath) setIndex(found);
      })
      .catch(() => {
        // Перечень не собрался — предлагаются только метки самой заметки.
      });
    return () => {
      alive = false;
    };
  }, [client, notePath, dependencies]);

  return index;
}
