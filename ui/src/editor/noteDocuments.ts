import type { OpenText } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import {
  NoteDocumentImpl,
  type NoteDocument,
  type NoteDocumentHooks,
} from "./noteDocument.js";

// Реестр документов на путь (фича 014, contracts/frontend-workspace.md).
//
// Один на открытый vault: окно создаёт его вместе с клиентом. Документ живёт,
// пока у него есть держатели (виды, потребители текста), пока его путь «закреплён»
// открытой вкладкой из недавних или пока в нём есть несохранённое. Всё остальное
// выгружается и при возврате к вкладке перечитывается с диска (Edge Case «50 и
// больше вкладок»): память не растёт с числом вкладок.

export class NoteDocumentRegistry {
  private readonly documents = new Map<string, NoteDocumentImpl>();
  private pinned: ReadonlySet<string> = new Set();
  private readonly hooks: NoteDocumentHooks;

  constructor(
    private readonly client: VaultClient,
    hooks: NoteDocumentHooks = {},
  ) {
    this.hooks = {
      ...hooks,
      onSettled: (document) => {
        this.maybeUnload(document as NoteDocumentImpl);
      },
    };
  }

  /** Документ по пути: создаётся и читается с диска при первом обращении. */
  acquire(path: string): NoteDocument {
    let document = this.documents.get(path);
    if (document === undefined) {
      document = new NoteDocumentImpl(this.client, path, this.hooks);
      this.documents.set(path, document);
      void document.load();
    }
    document.holders += 1;
    return document;
  }

  /** Отпустить документ, взятый `acquire`. Лишний вызов безвреден. */
  release(document: NoteDocument): void {
    const impl = document as NoteDocumentImpl;
    if (impl.holders > 0) impl.holders -= 1;
    this.maybeUnload(impl);
  }

  /** Уже загруженный документ, если он есть. Новый не создаёт. */
  get(path: string): NoteDocument | undefined {
    return this.documents.get(path);
  }

  /** Заметку переименовали: тот же документ под новым путём (вкладки не перечитывают текст). */
  rename(from: string, to: string): void {
    const document = this.documents.get(from);
    if (document === undefined || from === to) return;
    // Документ под новым путём уже есть — его перечитают, а старый уйдёт сам.
    if (this.documents.has(to)) return;
    this.documents.delete(from);
    document.path = to;
    this.documents.set(to, document);
  }

  /** Пути, документы которых держатся без видов (вкладки из недавних). */
  pin(paths: Iterable<string>): void {
    const next = new Set(paths);
    const released = [...this.pinned].filter((path) => !next.has(path));
    this.pinned = next;
    for (const path of released) {
      const document = this.documents.get(path);
      if (document !== undefined) this.maybeUnload(document);
    }
  }

  /** Перечитать документ, если он загружен и в нём нет несохранённого. */
  reload(path: string): void {
    void this.documents.get(path)?.reload();
  }

  /**
   * Тексты загруженных заметок — для связей (013, research.md §10): места и
   * фрагменты считаются по тому, что видно в редакторе, а не по диску.
   */
  openTexts(): OpenText[] {
    return [...this.documents.values()]
      .filter((document) => document.snapshot().read === "ready")
      .map((document) => ({ path: document.path, text: document.text() }));
  }

  /** Есть ли среди загруженных документов с несохранённым. */
  hasDirty(): boolean {
    return [...this.documents.values()].some((document) => document.isDirty());
  }

  private maybeUnload(document: NoteDocumentImpl): void {
    if (document.holders > 0 || document.viewCount > 0) return;
    if (this.pinned.has(document.path) || document.isDirty()) return;
    if (this.documents.get(document.path) !== document) return;
    this.documents.delete(document.path);
    document.dispose();
  }

  /** Окно закрывается или vault сменился: таймеры и регистрации снимаются. */
  dispose(): void {
    for (const document of this.documents.values()) document.dispose();
    this.documents.clear();
  }
}
