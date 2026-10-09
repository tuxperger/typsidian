import init from "./wasm-core/typsidian_core.js";

let wasmInitPromise: Promise<unknown> | null = null;

/**
 * Инициализация wasm-ядра — одна на страницу, и звать её нужно только отсюда.
 *
 * Собственная защита `init()` у wasm-bindgen (`if (wasm !== undefined)`)
 * срабатывает лишь после загрузки: два вызова, пришедшие, пока первый ещё ждёт
 * `fetch`, создают два экземпляра модуля. Второй подменяет память первого, и
 * колбэки уже ожидающих промисов исполняются поверх чужой памяти — снаружи это
 * выглядит как «FnOnce called more than once».
 */
export function ensureWasmInitialized(): Promise<unknown> {
  if (!wasmInitPromise) wasmInitPromise = init();
  return wasmInitPromise;
}
