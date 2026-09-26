"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "../icons/Icons.js";
import type { VaultClient } from "../vault-client.js";
import type {
  ElementSpec,
  FieldError,
  FieldProblem,
  FieldValue,
  ParamKind,
  ParamSpec,
  ParamValue,
  ParsedElement,
  PlanEditError,
  TextEdit,
} from "../wasm-core/typsidian_core.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { describeUnavailable } from "./useElementAtCaret.js";

// Форма параметров элемента Typst (фича 008, макет «Typsidian Вставка» →
// § Параметры блока).
//
// Одна форма на весь каталог и на оба случая — вставить новый элемент и
// поправить готовый. Поля она не знает сама: строит их по схеме элемента,
// которую отдаёт ядро (FR-001a). Решений о языке здесь нет вовсе (принцип I):
// кавычки, скобки, проверку значений, текст «Будет вставлено» и план правки
// делает ядро; форма держит набранное и показывает ответ.

export type ParamFormMode =
  | { kind: "insert" }
  | { kind: "edit"; parsed: ParsedElement };

export interface ParamFormProps {
  client: VaultClient;
  spec: ElementSpec;
  /** Название элемента из каталога — «Рисунок», «Колонки». */
  title: string;
  mode: ParamFormMode;
  /** Текст заметки сейчас: по нему ядро проверяет метки и сверяет элемент. */
  noteText: () => string;
  /**
   * Почему применять нельзя вовсе (FR-019): папка только для чтения,
   * неразрешённый конфликт. `null` — можно.
   */
  blockedReason?: string | null;
  onInsert: (text: string) => void;
  onApply: (edits: TextEdit[]) => void;
  onCancel: () => void;
}

/** Поле формы: набранный текст и то, как ядро должно его понимать. */
interface FieldState {
  text: string;
  /** Поле «как есть»: текст — готовая запись Typst (FR-012a). */
  expression: boolean;
  /** Пользователь трогал поле: только такие и значения из текста уходят в ядро. */
  touched: boolean;
}

/** Параметр вместе с полным именем поля (`body.width` у вложенного). */
interface FieldDef {
  name: string;
  param: ParamSpec;
}

export function ParamForm({
  client,
  spec,
  title,
  mode,
  noteText,
  blockedReason = null,
  onInsert,
  onApply,
  onCancel,
}: ParamFormProps): React.JSX.Element {
  const editing = mode.kind === "edit";
  const parsed = mode.kind === "edit" ? mode.parsed : null;
  const params = spec.params ?? [];
  const labelPrefix = spec.label?.prefix ?? null;

  // Вложенный элемент (изображение в рисунке) показывается своими полями, если
  // ядро его разобрало — при вставке по `initial_inner`, при правке по `inner`.
  const showInner = spec.inner !== undefined && (parsed ? parsed.inner : spec.initial_inner === true);

  const defs = useMemo<{ main: FieldDef[]; rest: FieldDef[] }>(() => {
    const all: FieldDef[] = [];
    for (const param of params) {
      if (showInner && spec.inner?.param === param.name) {
        for (const inner of spec.inner.params ?? []) {
          all.push({ name: `${param.name}.${inner.name}`, param: inner });
        }
        continue;
      }
      all.push({ name: param.name, param });
    }
    return {
      main: all.filter((def) => def.param.main),
      rest: all.filter((def) => !def.param.main),
    };
  }, [params, showInner, spec.inner]);

  const [fields, setFields] = useState<Record<string, FieldState>>(() =>
    initialFields(parsed ? parsed.values : (spec.initial ?? [])),
  );

  const initialLabel = splitLabel(parsed?.label ?? null, labelPrefix);
  const [labelText, setLabelText] = useState(initialLabel.suffix);
  const labelFull = joinLabel(labelText, initialLabel.prefixed ? labelPrefix : null);

  // «Все параметры» раскрыты сами, если в правимом элементе задан хоть один
  // не-основной (FR-012b): пользователь видит всё, что записано в тексте.
  const [showAll, setShowAll] = useState(() =>
    defs.rest.some((def) => fields[def.name] !== undefined && !fields[def.name]?.touched),
  );

  const [errors, setErrors] = useState<FieldError[]>([]);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [preview, setPreview] = useState<string>("");
  const [edits, setEdits] = useState<TextEdit[] | null>(null);
  const [rendered, setRendered] = useState<string | null>(null);

  useDismissOnEscape(onCancel);

  // Служебные поля начальных значений (`@body` — текст заголовка или кода)
  // формой не показываются, но уходят в ядро как есть.
  const pseudo = useMemo<FieldValue[]>(
    () =>
      (spec.prefill ?? [])
        .filter((prefill) => prefill.name.startsWith("@"))
        .map((prefill) => ({ name: prefill.name, source: prefill.value })),
    [spec.prefill],
  );

  const request = useMemo<FieldValue[]>(() => {
    const out: FieldValue[] = [...pseudo];
    for (const def of [...defs.main, ...defs.rest]) {
      const state = fields[def.name];
      if (!state) continue;
      if (state.expression || !plainKind(def.param.kind)) {
        out.push({ name: def.name, source: state.text });
      } else {
        out.push({ name: def.name, plain: state.text });
      }
    }
    return out;
  }, [fields, defs, pseudo]);

  // «Будет вставлено» — на каждое изменение (FR-004): это тот самый текст, что
  // попадёт в заметку (FR-005), поэтому его строит ядро, а не форма.
  const generation = useRef(0);
  useEffect(() => {
    const current = ++generation.current;
    void (async () => {
      if (!parsed) {
        const outcome = await client.renderElement({
          id: spec.id,
          fields: request,
          label: labelFull ?? undefined,
          taken_labels: [],
          note: noteText(),
        });
        if (current !== generation.current) return;
        if (outcome.kind === "Rendered") {
          setErrors([]);
          setRendered(outcome.element.text);
          setPreview(outcome.element.text);
        } else {
          setErrors(outcome.errors);
          setRendered(null);
        }
        return;
      }

      const text = noteText();
      const outcome = await client.planElementEdit({
        text,
        parsed,
        fields: request,
        label: labelFull ?? undefined,
      });
      if (current !== generation.current) return;
      if (outcome.kind === "Planned") {
        setErrors([]);
        setRefusal(null);
        setEdits(outcome.edits);
        setPreview(replaced(text, parsed, outcome.edits));
      } else {
        setEdits(null);
        if (outcome.error.kind === "Invalid") {
          setErrors(outcome.error.errors);
          setRefusal(null);
        } else {
          setErrors([]);
          setRefusal(describeRefusal(outcome.error));
        }
      }
    })();
  }, [client, spec.id, request, labelFull, parsed, noteText]);

  const canApply =
    blockedReason === null &&
    refusal === null &&
    errors.length === 0 &&
    (editing ? edits !== null : rendered !== null);

  const apply = (): void => {
    if (!canApply) return;
    if (editing) {
      if (edits === null) return;
      if (edits.length === 0) onCancel(); // ничего не менялось (FR-009)
      else onApply(edits);
    } else if (rendered !== null) {
      onInsert(rendered);
    }
  };

  const setField = (name: string, text: string, expression: boolean): void => {
    setFields((current) => ({ ...current, [name]: { text, expression, touched: true } }));
  };

  const errorOf = (name: string): FieldProblem | undefined =>
    errors.find((error) => error.name === name)?.problem;

  const labelError = errorOf("label");
  const firstField = defs.main[0]?.name ?? defs.rest[0]?.name ?? null;

  const renderField = (def: FieldDef): React.JSX.Element => (
    <ParamField
      key={def.name}
      def={def}
      state={fields[def.name]}
      problem={errorOf(def.name)}
      autoFocus={def.name === firstField}
      onChange={setField}
    />
  );

  return (
    <div
      className="ts-dialog ts-dialog--params"
      role="dialog"
      aria-modal="true"
      aria-label={`${editing ? "Параметры" : "Вставить"}: ${title}`}
      onKeyDown={(event) => {
        // ⌘↵ применяет из любого поля (FR-017); обычный Enter оставлен полям.
        if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
          event.preventDefault();
          apply();
        }
      }}
    >
      <div className="ts-dialog__head ts-params__head">
        <Icon name="terminal" size={14} className="ts-params__icon" />
        <span className="ts-params__title">
          {editing ? "Параметры" : "Вставить"}: {title}
        </span>
        <span className="ts-params__tag">
          {spec.syntax === "set" ? `#set ${spec.typst}` : `#${spec.typst}`}
        </span>
        <button type="button" className="ts-btn ts-btn--icon" aria-label="Закрыть" onClick={onCancel}>
          <Icon name="x" size={12} />
        </button>
      </div>

      <div className="ts-dialog__body ts-params">
        {defs.main.map(renderField)}

        {labelPrefix !== null && (
          <label className="ts-field">
            <span className="ts-field__label">Метка</span>
            <div className="ts-labelfield">
              {initialLabel.prefixed && (
                <span className="ts-labelfield__prefix">{labelPrefix}:</span>
              )}
              <input
                className={`ts-field__input${labelError ? " ts-field__input--bad" : ""}`}
                type="text"
                value={labelText}
                placeholder="без метки"
                aria-invalid={labelError !== undefined}
                onChange={(event) => setLabelText(event.target.value)}
              />
            </div>
            {labelError && (
              <span className="ts-field__hint ts-field__hint--bad" role="alert">
                {describeProblem(labelError)}
              </span>
            )}
          </label>
        )}

        {defs.rest.length > 0 && (
          <details
            className="ts-params__all"
            open={showAll}
            onToggle={(event) => setShowAll(event.currentTarget.open)}
          >
            <summary className="ts-params__allhead">
              Все параметры <span className="ts-params__count">{defs.rest.length}</span>
            </summary>
            {showAll && <div className="ts-params__allbody">{defs.rest.map(renderField)}</div>}
          </details>
        )}

        {refusal && (
          <p className="ts-field__hint ts-field__hint--bad" role="alert">
            {refusal}
          </p>
        )}
        {blockedReason && (
          <p className="ts-field__hint ts-field__hint--bad" role="alert">
            {blockedReason}
          </p>
        )}

        <div className="ts-params__output">
          <span className="ts-eyebrow">{editing ? "Будет заменено на" : "Будет вставлено"}</span>
          <pre className="ts-insert__code">{preview}</pre>
        </div>
      </div>

      <div className="ts-dialog__actions ts-params__foot">
        {labelFull && <span className="ts-params__note">ссылка @{labelFull}</span>}
        <span className="ts-spacer" />
        <button type="button" className="ts-btn" onClick={onCancel}>
          Отмена
        </button>
        <button type="button" className="ts-btn ts-btn--accent" disabled={!canApply} onClick={apply}>
          {editing ? "Применить" : "Вставить"}
          <span className="ts-params__keys">⌘↵</span>
        </button>
      </div>
    </div>
  );
}

/** Одно поле по виду значения параметра (FR-014: список — выбором). */
function ParamField({
  def,
  state,
  problem,
  autoFocus,
  onChange,
}: {
  def: FieldDef;
  state: FieldState | undefined;
  problem: FieldProblem | undefined;
  autoFocus: boolean;
  onChange: (name: string, text: string, expression: boolean) => void;
}): React.JSX.Element {
  const { param, name } = def;
  const text = state?.text ?? "";
  const expression = state?.expression ?? param.kind.type === "expression";
  const base = baseKind(param.kind);
  const placeholder = param.default ? `по умолчанию ${param.default}` : "";
  const bad = problem !== undefined;
  const inputClass = `ts-field__input${expression ? " ts-field__input--code" : ""}${bad ? " ts-field__input--bad" : ""}`;

  let control: React.JSX.Element;
  if (!expression && (base.type === "choice" || base.type === "bool")) {
    const options =
      base.type === "bool"
        ? [
            { value: "true", label: "да" },
            { value: "false", label: "нет" },
          ]
        : base.options.map((option) => ({ value: option, label: option.replace(/^"|"$/g, "") }));
    const extra = wrappers(param.kind).map((value) => ({ value, label: value }));
    control = (
      <select
        className={inputClass}
        value={text}
        autoFocus={autoFocus}
        aria-invalid={bad}
        onChange={(event) => onChange(name, event.target.value, false)}
      >
        <option value="">{param.default ? `по умолчанию (${param.default})` : "—"}</option>
        {[...extra, ...options].map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    );
  } else {
    control = (
      <input
        className={inputClass}
        type="text"
        value={text}
        placeholder={placeholder}
        autoFocus={autoFocus}
        spellCheck={!expression && plainKind(param.kind)}
        aria-invalid={bad}
        onChange={(event) => onChange(name, event.target.value, expression)}
      />
    );
  }

  return (
    <label className="ts-field">
      <span className="ts-field__label">
        {param.title}
        {param.required && !param.variadic && <span className="ts-field__req"> *</span>}
      </span>
      {control}
      {expression && param.kind.type !== "expression" && (
        <span className="ts-field__hint">
          Выражение Typst — сохранится как есть, пока вы его не измените.
        </span>
      )}
      {bad && (
        <span className="ts-field__hint ts-field__hint--bad" role="alert">
          {describeProblem(problem)}
        </span>
      )}
    </label>
  );
}

function initialFields(values: ParamValue[]): Record<string, FieldState> {
  const out: Record<string, FieldState> = {};
  for (const value of values) {
    out[value.name] = {
      text: value.expression ? value.source : value.plain,
      expression: value.expression,
      touched: false,
    };
  }
  return out;
}

/** Вид значения без обёрток «авто»/«нет». */
function baseKind(kind: ParamKind): ParamKind {
  return kind.type === "auto-or" || kind.type === "none-or" ? baseKind(kind.inner) : kind;
}

/** Значения-обёртки, которые предлагаются в списке выбора: `auto`, `none`. */
function wrappers(kind: ParamKind): string[] {
  if (kind.type === "auto-or") return ["auto", ...wrappers(kind.inner)];
  if (kind.type === "none-or") return ["none", ...wrappers(kind.inner)];
  return [];
}

/**
 * Поле набирается «как текст» — строка или содержимое: кавычки и скобки к нему
 * добавит ядро. Остальные поля — готовая запись Typst (`2cm`, `center`).
 */
function plainKind(kind: ParamKind): boolean {
  const base = baseKind(kind);
  return base.type === "string" || base.type === "content";
}

function splitLabel(
  label: string | null,
  prefix: string | null,
): { suffix: string; prefixed: boolean } {
  if (label === null) return { suffix: "", prefixed: prefix !== null };
  if (prefix !== null && label.startsWith(`${prefix}:`)) {
    return { suffix: label.slice(prefix.length + 1), prefixed: true };
  }
  // Метка с чужой приставкой показывается целиком и не переписывается молча.
  return { suffix: label, prefixed: false };
}

function joinLabel(suffix: string, prefix: string | null): string | null {
  const trimmed = suffix.trim();
  if (trimmed === "") return null;
  return prefix === null ? trimmed : `${prefix}:${trimmed}`;
}

/** Элемент после правки — для «Будет заменено на». */
function replaced(text: string, parsed: ParsedElement, edits: TextEdit[]): string {
  let out = "";
  let cursor = parsed.from;
  let end = parsed.to;
  for (const edit of edits) {
    if (edit.from < parsed.from || edit.to > parsed.to) {
      // Замена у границы (метка после элемента) — расширяем показ.
      end = Math.max(end, edit.to);
    }
  }
  for (const edit of edits) {
    out += text.slice(cursor, edit.from) + edit.insert;
    cursor = edit.to;
  }
  return out + text.slice(cursor, end);
}

export function describeProblem(problem: FieldProblem | undefined): string {
  switch (problem?.kind) {
    case "Required":
      return "Обязательное поле.";
    case "Unparsable":
      return "Запись не разбирается как значение Typst.";
    case "WrongKind":
      return "Значение не того вида — например, строка вместо длины.";
    case "NotAnOption":
      return "Такого значения нет среди допустимых.";
    case "LabelTaken":
      return "Такая метка уже есть в этой заметке — ссылки на неё сломаются.";
    case "LabelInvalid":
      return "В метке допустимы буквы, цифры и знаки - _ . :";
    default:
      return "";
  }
}

function describeRefusal(error: PlanEditError): string {
  switch (error.kind) {
    case "Stale":
      return "Текст элемента изменился, пока форма была открыта. Откройте параметры заново — правка поверх не записана.";
    case "Gone":
      return "Элемента на прежнем месте больше нет.";
    case "Unavailable":
      return describeUnavailable(error.reason);
    case "Invalid":
      return "";
  }
}
