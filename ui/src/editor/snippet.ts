import {
  EditorSelection,
  Prec,
  StateEffect,
  StateField,
  type Extension,
} from "@codemirror/state";
import { Decoration, EditorView, keymap } from "@codemirror/view";

// Вставка элемента Typst как сниппета с полями (макет «Typsidian Вставка» →
// § Меню вставки: «⇥ поля сниппета»).
//
// Почему своя реализация, а не `snippet()` из `@codemirror/autocomplete`: тот
// пакет — это вся система автодополнения (список, скоринг, источники), а нужен
// из неё один механизм табстопов. Здесь его ровно столько, сколько названо в
// handoff: поля, переход по ⇥ и выход по Escape. Формат шаблона поэтому тоже
// свой и намеренно беднее: `${значение}` — поле с содержимым по умолчанию,
// вложенности и повторов нет.
//
// ВАЖНО про шаблоны: они пишутся обычными кавычками, а не обратными. В
// шаблонной строке JS `${…}` — подстановка, и поле сниппета исчезло бы ещё до
// разбора.

/** Поле сниппета — диапазон в документе, который заполняет пользователь. */
export interface SnippetField {
  from: number;
  to: number;
}

export interface ParsedSnippet {
  /** Текст без разметки полей — то, что попадёт в документ. */
  text: string;
  /** Поля, смещения внутри `text`. */
  fields: SnippetField[];
}

/**
 * Разбирает шаблон на текст и поля.
 *
 * `wrap` — выделенный в редакторе текст: он подставляется в первое поле, чтобы
 * «выделить и обернуть» работало так же, как в любом редакторе (выделил абзац,
 * выбрал «Цитата» — абзац оказался внутри цитаты, а не стёрт ею).
 */
export function parseSnippet(template: string, wrap?: string): ParsedSnippet {
  // Регулярное выражение создаётся здесь, а не в модуле: с флагом `g` оно
  // хранит `lastIndex`, и общий экземпляр пропускал бы поля через раз.
  const pattern = /\$\{([^}]*)\}/g;
  const fields: SnippetField[] = [];
  let text = "";
  let last = 0;

  for (
    let match = pattern.exec(template);
    match !== null;
    match = pattern.exec(template)
  ) {
    text += template.slice(last, match.index);
    const value = fields.length === 0 && wrap !== undefined ? wrap : (match[1] ?? "");
    fields.push({ from: text.length, to: text.length + value.length });
    text += value;
    last = match.index + match[0].length;
  }

  text += template.slice(last);
  return { text, fields };
}

/** Смена набора полей: и установка после вставки, и снятие по Escape. */
const setFields = StateEffect.define<SnippetField[]>();

const fieldMark = Decoration.mark({ class: "ts-snippet-field" });

/**
 * Поля активного сниппета.
 *
 * Диапазоны переносятся через каждую правку: пользователь печатает **внутри**
 * поля, и без переноса следующее ⇥ уводило бы курсор в место, где поле было до
 * набора. `-1` и `1` в `mapPos` дают полю расти вместе с набранным текстом, а
 * не оставаться прежней длины.
 */
const activeFields = StateField.define<SnippetField[]>({
  create: () => [],

  update(fields, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(setFields)) return effect.value;
    }
    if (fields.length === 0) return fields;

    const moved = transaction.docChanged
      ? fields.map((field) => ({
          from: transaction.changes.mapPos(field.from, -1),
          to: transaction.changes.mapPos(field.to, 1),
        }))
      : fields;

    // Курсор ушёл из полей — сниппет закончен. Иначе ⇥ продолжал бы прыгать по
    // местам, которые пользователь давно оставил, вместо обычного отступа.
    const head = transaction.newSelection.main.head;
    return moved.some((field) => head >= field.from && head <= field.to) ? moved : [];
  },

  provide: (field) =>
    EditorView.decorations.from(field, (fields) =>
      Decoration.set(
        fields
          // Пустое поле подсветить нечем: у метки нулевой длины нет места на
          // экране, а CodeMirror такие диапазоны запрещает.
          .filter((range) => range.to > range.from)
          .map((range) => fieldMark.range(range.from, range.to)),
        true,
      ),
    ),
});

/** Переход к соседнему полю; за последним сниппет заканчивается. */
function jumpField(step: 1 | -1) {
  return (view: EditorView): boolean => {
    const fields = view.state.field(activeFields, false) ?? [];
    if (fields.length === 0) return false;

    const head = view.state.selection.main.head;
    const current = fields.findIndex((field) => head >= field.from && head <= field.to);
    const target = fields[current + step];

    if (!target) {
      const edge = step === 1 ? fields[fields.length - 1]?.to : fields[0]?.from;
      view.dispatch({
        selection: EditorSelection.cursor(edge ?? head),
        effects: setFields.of([]),
      });
      return true;
    }

    view.dispatch({
      selection: EditorSelection.range(target.from, target.to),
      // Набор полей объявляется заново: без этого переход к соседнему полю
      // выглядел бы для `activeFields` как уход курсора наружу.
      effects: setFields.of(fields),
      scrollIntoView: true,
    });
    return true;
  };
}

/** Escape снимает поля, не трогая текст: набранное остаётся как есть. */
function dropFields(view: EditorView): boolean {
  if ((view.state.field(activeFields, false) ?? []).length === 0) return false;
  view.dispatch({ effects: setFields.of([]) });
  return true;
}

/**
 * Расширение редактора: подсветка полей и переходы по ним.
 *
 * Приоритет высокий, но команды возвращают `false`, когда сниппета нет, —
 * ⇥ вне сниппета обязан работать как раньше (уводить фокус из редактора), иначе
 * клавиатурная навигация по странице ломается.
 */
export const snippetFields: Extension = [
  activeFields,
  Prec.high(
    keymap.of([
      { key: "Tab", run: jumpField(1), shift: jumpField(-1) },
      { key: "Escape", run: dropFields },
    ]),
  ),
];

export interface InsertOptions {
  /**
   * Блочный элемент: `= Заголовок` посреди абзаца заголовком не станет,
   * поэтому такой сниппет начинает строку.
   */
  block?: boolean;
  /**
   * Откуда заменять текст — смещение косой черты, которой вызвали меню
   * (макет § Меню вставки: вызов по `/`).
   *
   * Без этого `/` осталась бы в тексте перед вставленным элементом: меню
   * открылось по ней, а вставка пришла бы следом.
   */
  replaceFrom?: number;
  /**
   * До какого места заменять. Нужна правке готового блока: там заменяется весь
   * `#figure(…) <fig:…>`, а не кусок от каретки.
   *
   * Без неё замена идёт до курсора — так ведёт себя вызов по `/`.
   */
  replaceTo?: number;
}

/** Где стоит курсор среди полей активного сниппета (для статус-бара). */
export interface SnippetFieldState {
  /** Номер поля с единицы — так же, как он называется пользователю. */
  index: number;
  count: number;
}

/**
 * Поле, в котором сейчас курсор; `null` — сниппет не активен.
 *
 * Нужно статус-бару: без «поле 2 из 3» подсветка полей говорит только, что
 * что-то выделено, а сколько их ещё впереди — нет (макет § Панель вставки +
 * состояние полей).
 */
export function snippetFieldState(view: EditorView): SnippetFieldState | null {
  const fields = view.state.field(activeFields, false) ?? [];
  if (fields.length === 0) return null;
  const head = view.state.selection.main.head;
  const at = fields.findIndex((field) => head >= field.from && head <= field.to);
  return at === -1 ? null : { index: at + 1, count: fields.length };
}

/**
 * Вставляет шаблон в позицию курсора и встаёт в первое поле.
 *
 * Выделенный текст заменяется вставкой и переезжает в первое поле — см.
 * `parseSnippet`.
 */
export function insertSnippet(
  view: EditorView,
  template: string,
  options: InsertOptions = {},
): void {
  const { state } = view;
  const range = state.selection.main;
  const selected = state.sliceDoc(range.from, range.to);
  const parsed = parseSnippet(template, selected === "" ? undefined : selected);

  // Замена начинается с косой черты, если меню вызвали ею, — иначе с обычного
  // начала выделения. Значение подрезается по документу: пока меню было
  // открыто, текст мог измениться (внешняя правка, принятая версия).
  const from = Math.max(0, Math.min(options.replaceFrom ?? range.from, range.from));
  const to = Math.min(
    state.doc.length,
    Math.max(options.replaceTo ?? range.to, from),
  );

  const lineStart = state.doc.lineAt(from).from;
  const needsBreak = options.block === true && state.sliceDoc(lineStart, from).trim() !== "";
  const insert = needsBreak ? `\n${parsed.text}` : parsed.text;
  const origin = from + (needsBreak ? 1 : 0);

  const fields = parsed.fields.map((field) => ({
    from: origin + field.from,
    to: origin + field.to,
  }));
  const first = fields[0];

  view.dispatch({
    changes: { from, to, insert },
    selection: first
      ? EditorSelection.range(first.from, first.to)
      : EditorSelection.cursor(from + insert.length),
    effects: setFields.of(fields),
    scrollIntoView: true,
    userEvent: "input.complete",
  });
  view.focus();
}
