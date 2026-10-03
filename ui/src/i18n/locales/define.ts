/**
 * Общее для каталогов (фича 010): форма записи и помощники форм при числах.
 *
 * Каталог каждого языка разбит по областям интерфейса (`en/tree.ts`,
 * `ru/tree.ts`…), а `en.ts`/`ru.ts` их собирают. Разбиение — только ради
 * читаемости: тип [`Translation`] требует от области перевода те же ключи и
 * те же параметры, что у эталона, так что забытый ключ по-прежнему роняет `tsc`.
 */

import { plural } from "../format.js";

/** Значение ключа: строка или функция от типизированных параметров. */
export type Entry = string | ((params: never) => string);
export type Entries = Record<string, Entry>;

type Widen<T> = T extends string ? string : T;

/** Перевод области: ключи и параметры эталона, тексты — свои. */
export type Translation<T extends Entries> = { [K in keyof T]: Widen<T[K]> };

/** «1 note» / «3 notes». `#` — число, оформленное по правилам языка. */
export const enN = (count: number, one: string, other: string): string =>
  plural("en", count, { one, other });

/** «1 заметка» / «3 заметки» / «9 заметок». */
export const ruN = (count: number, one: string, few: string, many: string): string =>
  plural("ru", count, { one, few, many, other: few });
