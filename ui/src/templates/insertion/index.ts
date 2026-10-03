import type {
  InsertionEdit,
  InsertionPoint,
  Template,
} from "../../wasm-core/typsidian_core.js";
import type { VaultClient } from "../../vault-client.js";
import type { Phrase } from "../../i18n/index.js";

// Порт вставки заготовки в открытую заметку — граница гейта принципа V
// (`specs/003-templates-snippets/contracts/insertion-target.md`).
//
// Отдельный модуль, а не функция внутри галереи, потому что здесь проходит
// граница незакрытого гейта конституции: реализации для двух из трёх режимов
// редактора планировать нельзя до отдельной сессии `/speckit-clarify` по мосту
// к Neovim. Подкаталог `insertion/` заведён сразу, при единственной реализации,
// чтобы эта граница была видна в раскладке файлов, а не только в документе.
//
// **Правило для реализаторов**: галерея вызывает вставку только через этот
// порт, даже пока реализация одна. Соблазн дёрнуть редактор напрямую «пока
// режим один» разрушает ровно ту границу, ради которой порт написан: после
// clarify пришлось бы переписывать галерею вместо того, чтобы дописать два
// файла рядом с `plainInputTarget.ts`.

/** Почему вставка не состоялась. */
export type InsertError =
  /** Редактора сейчас нет: заметка не открыта или панель размонтирована. */
  | { kind: "NoEditor" }
  /** Заготовка не прочиталась — причину назвало ядро; слова — при показе. */
  | { kind: "TemplateUnreadable"; reason: Phrase };

export type InsertResult = { ok: true } | { ok: false; error: InsertError };

export type { InsertionEdit, InsertionPoint };

/**
 * Цель вставки — один режим редактора.
 *
 * Порт принимает **готовую правку**, а не содержимое: что именно заменить и
 * куда встанет курсор, считает ядро (`planInsertion`), потому что три режима
 * редактора обязаны согласиться об этом, а не договариваться каждый со своим
 * редактором.
 *
 * Обязательства, одинаковые для всех реализаций
 * (contracts/insertion-target.md):
 *
 * 1. **Одна отмена возвращает текст целиком.** Не «примерно одна» и не «одна на
 *    короткой заготовке»: вставка обязана быть одной единицей истории отмены.
 * 2. **История отмены не обнуляется.** После вставки предыдущие правки
 *    пользователя остаются откатываемыми — реализация «прочитать всё, склеить,
 *    записать обратно» это требование нарушает.
 * 3. **Выделение заменяется** — поведение обычной вставки из буфера обмена.
 * 4. **Фокус остаётся в редакторе, курсор в предсказуемом месте**, одинаковом
 *    от вставки к вставке.
 * 5. **Содержимое вставляется дословно**: подстановок нет, потому что
 *    параметризация вне скоупа.
 */
export interface InsertionTarget {
  /**
   * Текущая точка вставки: позиция курсора и выделение, если оно есть.
   *
   * Смещения — в кодовых единицах UTF-16, той же мере, в которой их считает
   * редактор и ядро (`InsertionPoint` в `templates/mod.rs`).
   */
  insertionPoint(): InsertionPoint | null;
  /** Длина текста — ядру, чтобы прижать смещения к границам документа. */
  textLength(): number;
  /** Применить правку одной отменяемой единицей. */
  apply(edit: InsertionEdit): InsertResult;
}

/**
 * Применение заготовки: прочитать содержимое, спросить у ядра правку, отдать её
 * цели.
 *
 * Правил вставки здесь нет — только порядок. Чтение идёт первым: обратный
 * порядок означал бы правку текста ради содержимого, которое могло не
 * прочитаться. Точка вставки снимается **после** чтения, непосредственно перед
 * планированием, чтобы длинное ожидание файла не пришлось на промежуток между
 * снятием позиции курсора и правкой.
 */
export async function insertTemplate(
  target: InsertionTarget,
  client: VaultClient,
  template: Template,
): Promise<InsertResult> {
  let content: string;
  try {
    content = await client.readTemplate(template.path);
  } catch (cause) {
    return {
      ok: false,
      error: { kind: "TemplateUnreadable", reason: describeReadFailure(cause) },
    };
  }

  const point = target.insertionPoint();
  if (point === null) return { ok: false, error: { kind: "NoEditor" } };

  const edit = await client.planInsertion(target.textLength(), point, content);
  return target.apply(edit);
}

/** Причина отказа чтения заготовки словами (FR-017). */
export function describeReadFailure(cause: unknown): Phrase {
  const kind =
    typeof cause === "object" && cause !== null && "kind" in cause
      ? (cause as { kind: unknown }).kind
      : null;
  switch (kind) {
    case "NotFound":
      return (t) => t("templates.read.notFound");
    case "Unreadable":
      return (t) => t("templates.read.unreadable");
    case "Io":
      return (t) => t("templates.read.io");
    default:
      return () => String(cause);
  }
}
