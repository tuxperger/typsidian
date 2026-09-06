import type { CompiledDocument } from "./wasm-core/typsidian_core.js";

// Поверхность предпросмотра, которой пользуется общий UI (фича 005).
//
// Отдельно от `VaultClient` по той же причине, по которой в ядре это отдельный
// объект: предпросмотр не читает vault и не владеет им. На web это компилятор в
// том же процессе, на desktop — команда Tauri; общий UI не знает, какой из них
// используется (принцип II).

export interface PreviewClient {
  /**
   * Собрать заметку из текста редактора.
   *
   * `revision` — монотонная версия текста; ядро помечает ею результат, чтобы
   * более старая сборка не вытеснила более новую (FR-009).
   */
  compile(
    notePath: string,
    text: string,
    revision: number,
  ): Promise<CompiledDocument>;
}
