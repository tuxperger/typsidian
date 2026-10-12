"use client";

import { useEffect, useState, useSyncExternalStore } from "react";
import type { NoteDocument, DocumentSnapshot } from "./noteDocument.js";
import type { NoteDocumentRegistry } from "./noteDocuments.js";

/** Пока документа нет (путь не назван или он ещё берётся), потребитель видит пустой снимок. */
const NO_DOCUMENT: DocumentSnapshot = {
  text: "",
  read: "loading",
  save: { state: "Clean" },
  revision: 0,
};

const noSubscription = (): (() => void) => () => undefined;

/**
 * Документ по пути на время жизни компонента (держатель в реестре).
 *
 * `null` на первом кадре: документ берётся эффектом. Смена пути переименованной
 * заметки отпускает и снова берёт тот же документ под новым ключом.
 */
export function useNoteDocument(
  registry: NoteDocumentRegistry | null,
  path: string | null,
): NoteDocument | null {
  const [document, setDocument] = useState<NoteDocument | null>(null);

  useEffect(() => {
    if (registry === null || path === null) {
      setDocument(null);
      return undefined;
    }
    const acquired = registry.acquire(path);
    setDocument(acquired);
    return () => registry.release(acquired);
  }, [registry, path]);

  return document;
}

/** Текст и состояние документа как внешнее хранилище React. */
export function useDocumentSnapshot(document: NoteDocument | null): DocumentSnapshot {
  return useSyncExternalStore(
    document === null ? noSubscription : (listener) => document.subscribe(listener),
    () => (document === null ? NO_DOCUMENT : document.snapshot()),
    () => NO_DOCUMENT,
  );
}
