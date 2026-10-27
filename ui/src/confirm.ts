/**
 * Подтверждение деструктивного действия — одна точка на весь интерфейс.
 *
 * По умолчанию это `window.confirm`: в браузере он синхронный и работает. В
 * оболочке рабочего стола — нет: `tauri-plugin-dialog` подменяет его
 * асинхронной версией поверх команды `plugin:dialog|confirm`, которой в самом
 * плагине нет (2.8.1), и вызов отклоняется — удаление заметки и переименование
 * заметки со ссылками молча обрывались (001 FR-027, FR-029). Поэтому оболочка
 * подставляет сюда своё подтверждение через [`setConfirmAction`] при запуске.
 *
 * Ответ всегда асинхронный: так у вызывающего нет соблазна написать
 * `if (confirm(…))`, где Promise оказался бы истинным и действие прошло бы без
 * согласия.
 */
export type ConfirmAction = (message: string) => Promise<boolean>;

let current: ConfirmAction = (message) => Promise.resolve(window.confirm(message));

/** Подтверждение оболочки; вызывается один раз, до монтирования интерфейса. */
export function setConfirmAction(confirm: ConfirmAction): void {
  current = confirm;
}

/** Спросить согласия пользователя; `false` — отказ или закрытое окно. */
export function confirmAction(message: string): Promise<boolean> {
  return current(message);
}
