"use client";

import { createContext, useContext } from "react";
import type { BookmarkTarget } from "../wasm-core/typsidian_core.js";

// «В закладки» для мест, до которых рабочая область не тянет свои обработчики
// (фича 032): меню вкладки и строки оглавления. Нет провайдера — пункта нет.

export interface BookmarkActions {
  add(target: BookmarkTarget, name: string): void;
  /** Раздел заметки, чьё оглавление показано; `null` — оглавления нет. */
  section: ((heading: string) => void) | null;
}

export const BookmarkActionsContext = createContext<BookmarkActions | null>(null);

export function useBookmarkActions(): BookmarkActions | null {
  return useContext(BookmarkActionsContext);
}
