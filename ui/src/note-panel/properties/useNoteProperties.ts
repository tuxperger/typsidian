"use client";

import { useEffect, useState } from "react";
import type { NoteProperties } from "../../wasm-core/typsidian_core.js";
import type { PropertiesClient } from "../../vault-client.js";

// Свойства открытой заметки для режима «Свойства» (фича 020,
// contracts/frontend-properties.md): `properties.read(text)` по остановке
// набора, как оглавление 015. Разбор записи — в ядре; здесь только ритм
// запросов и отбрасывание опоздавших ответов.

/** Пауза после последнего нажатия, по истечении которой текст читается заново. */
export const PROPERTIES_DEBOUNCE_MS = 250;

export interface NotePropertiesState {
  /** Последний прочитанный ответ; `null` — заметки нет или чтение ещё не вернулось. */
  properties: NoteProperties | null;
  /** Ответ относится к другому тексту, чем в редакторе сейчас. */
  pending: boolean;
}

export function useNoteProperties(
  client: PropertiesClient | null,
  notePath: string | null,
  text: string,
  enabled: boolean,
): NotePropertiesState {
  const [read, setRead] = useState<{ path: string; value: NoteProperties } | null>(null);
  const [pendingText, setPendingText] = useState<string | null>(null);

  useEffect(() => {
    if (!enabled || client === null || notePath === null) {
      setRead(null);
      setPendingText(null);
      return undefined;
    }
    let stale = false;
    setPendingText(text);
    const timer = window.setTimeout(() => {
      void client
        .read(text)
        .then((value) => {
          if (stale) return;
          setRead({ path: notePath, value });
          setPendingText(null);
        })
        .catch(() => {
          if (!stale) setPendingText(null);
        });
    }, PROPERTIES_DEBOUNCE_MS);
    return () => {
      stale = true;
      window.clearTimeout(timer);
    };
  }, [client, notePath, text, enabled]);

  return {
    properties: read !== null && read.path === notePath ? read.value : null,
    pending: pendingText !== null,
  };
}
