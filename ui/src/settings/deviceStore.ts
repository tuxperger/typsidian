import { WebStore } from "../wasm-core/typsidian_core.js";
import { createWasmCallQueue, type WasmCallQueue } from "../wasm-call-queue.js";
import { ensureWasmInitialized } from "../wasm-init.js";

/**
 * Единственный экземпляр хранилища настроек устройства в браузере.
 *
 * Владелец у `WebStore` обязан быть один — это требование не стиля, а границы
 * wasm-bindgen: его `async`-методы с `&mut self` держат заимствование объекта
 * до конца своей future, и два экземпляра поверх одного файла в OPFS вдобавок
 * перетирали бы записи друг друга, каждый своей копией значений.
 *
 * Пока хранилище открывала только панель настроек, владелец был очевиден —
 * `useSettings`. С появлением предложения открыть последний vault (FR-007)
 * хранилище понадобилось **до** панели: на экране приветствия, где её ещё нет.
 * Поэтому экземпляр и очередь живут здесь, а `useSettings` берёт готовые.
 */
let opening: Promise<WebStore> | null = null;

/**
 * Очередь доступа к хранилищу. Одна на экземпляр: любой код, зовущий методы
 * `WebStore` в обход неё, наложится на цикл опроса настроек и упадёт с
 * «recursive use of an object detected» (см. `wasm-call-queue.ts`).
 */
export const deviceStoreQueue: WasmCallQueue = createWasmCallQueue();

/** Хранилище настроек устройства; открывается один раз за жизнь страницы. */
export function deviceStore(): Promise<WebStore> {
  if (opening === null) {
    opening = (async () => {
      // Только общий `ensureWasmInitialized`: прямой `init()` параллельно с
      // чужим создаёт второй экземпляр модуля (см. `wasm-init.ts`).
      await ensureWasmInitialized();
      return WebStore.create();
    })();
  }
  return opening;
}
