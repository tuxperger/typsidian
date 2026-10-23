"use client";

import { useCallback, useEffect, useState } from "react";
import type {
  BookmarkPlace,
  BookmarkTarget,
  BookmarksView,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";

// Закладки открытого vault (фича 032). Правила — ядра (`bookmarks`): каждая
// правка отдаёт новый вид, здесь он только хранится до следующей.

export type BookmarkAddResult = "added" | "exists" | "failed";

export interface BookmarksState {
  view: BookmarksView | null;
  /** Отказ последней правки — код `BookmarkError`. */
  error: string | null;
  add(target: BookmarkTarget, name?: string | null): Promise<BookmarkAddResult>;
  remove(id: string): Promise<void>;
  rename(id: string, name: string): Promise<void>;
  move(id: string, place: BookmarkPlace): Promise<void>;
  groupCreate(name: string): Promise<BookmarksView | null>;
  groupRename(id: string, name: string): Promise<void>;
  groupToggle(id: string): Promise<void>;
  groupDelete(id: string): Promise<void>;
}

const codeOf = (cause: unknown): string => (typeof cause === "string" ? cause : "Io");

/**
 * `revision` — меняется, когда в vault что-то переименовали, перенесли или
 * удалили: вид перечитывается, пути и доступность целей приходят новыми.
 */
export function useBookmarks(client: VaultClient | null, revision: unknown): BookmarksState {
  const [view, setView] = useState<BookmarksView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (client === null) {
      setView(null);
      return;
    }
    let alive = true;
    void client.bookmarks
      .view()
      .then((next) => alive && setView(next))
      .catch(() => alive && setView(null));
    return () => {
      alive = false;
    };
  }, [client, revision]);

  const apply = useCallback(
    async (op: (bookmarks: VaultClient["bookmarks"]) => Promise<BookmarksView>) => {
      if (client === null) return null;
      try {
        const next = await op(client.bookmarks);
        setView(next);
        setError(null);
        return next;
      } catch (cause) {
        setError(codeOf(cause));
        return null;
      }
    },
    [client],
  );

  const add = useCallback(
    async (target: BookmarkTarget, name: string | null = null): Promise<BookmarkAddResult> => {
      if (client === null) return "failed";
      try {
        const outcome = await client.bookmarks.add(target, name);
        setView(outcome.view);
        setError(null);
        return outcome.kind === "Added" ? "added" : "exists";
      } catch (cause) {
        setError(codeOf(cause));
        return "failed";
      }
    },
    [client],
  );

  return {
    view,
    error,
    add,
    remove: async (id) => void (await apply((b) => b.remove(id))),
    rename: async (id, name) => void (await apply((b) => b.rename(id, name))),
    move: async (id, place) => void (await apply((b) => b.move(id, place))),
    groupCreate: (name) => apply((b) => b.groupCreate(name)),
    groupRename: async (id, name) => void (await apply((b) => b.groupRename(id, name))),
    groupToggle: async (id) => void (await apply((b) => b.groupToggle(id))),
    groupDelete: async (id) => void (await apply((b) => b.groupDelete(id))),
  };
}
