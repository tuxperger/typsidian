import type { FileKind, Note } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";

// Принцип I: правило «что такое доска» живёт в ядре
// (`packages/core/src/vault/mod.rs` → `classify`). Здесь только чтение того,
// что ядро уже сказало. Сверять расширение в TypeScript нельзя: это второй
// источник истины, и он разойдётся с ядром ровно в тот момент, когда правило
// усложнится — как оно уже усложнилось со снимками в папках `*.assets`.

/**
 * Доска ли эта запись дерева (FR-004 фичи 002).
 *
 * Вид берётся из самой записи: ядро проставляет его при обходе (`Note::new`).
 * Раньше здесь был вызов `WebBackend.fileKind(path)`, и он же был единственным
 * местом, из-за которого общий UI требовал WASM в том же процессе, — на desktop
 * ядро живёт за границей процесса, и синхронного ответа там не существует
 * (001 T113).
 */
export function isBoard(note: Note): boolean {
  return note.kind === "Board";
}

/**
 * Вид файла по пути, которого в дереве ещё нет, — например имени, которое
 * пользователь только что набрал в поле создания. Спрашивается у ядра.
 */
export async function fileKindOf(
  client: VaultClient,
  path: string,
): Promise<FileKind> {
  return client.fileKind(path);
}
