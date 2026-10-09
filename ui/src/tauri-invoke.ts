import { invoke as tauriInvoke, type InvokeArgs } from "@tauri-apps/api/core";

// Вызов команды Tauri с ответом в той же форме, в какой те же типы приходят из
// wasm.
//
// `Option::None` ядра через `serde_json` становится `null`, а через wasm —
// отсутствующим полем (`T | undefined` в типах `tsify`). Общий UI сверяется с
// `undefined`, поэтому `null` в поле проходит мимо проверки и роняет рендер при
// первом же обращении к его свойствам — так на рабочем столе пустели окно
// (010 T059, язык интерфейса) и панель синхронизации (011, `overview.remote`).
// Чинить это по одному полю значит ждать следующего; здесь разница снимается
// на границе, один раз для всех команд.

/**
 * Убирает поля со значением `null` — на любой глубине, включая элементы
 * массивов. Само значение верхнего уровня не трогается: `null` там — законный
 * ответ команды («ничего не найдено»).
 */
export function withoutNullFields(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(withoutNullFields);
  if (typeof value !== "object" || value === null) return value;
  const result: Record<string, unknown> = {};
  for (const [key, field] of Object.entries(value)) {
    if (field !== null) result[key] = withoutNullFields(field);
  }
  return result;
}

/** `invoke` из `@tauri-apps/api/core` с приведением ответа и отказа. */
export async function invoke<T = void>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    return withoutNullFields(await tauriInvoke(command, args)) as T;
  } catch (cause) {
    // Отказ — такое же значение ядра (`{ kind: … }`), и поля у него те же.
    throw withoutNullFields(cause);
  }
}
