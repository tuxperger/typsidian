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
  PropertyKeyInfo,
  PlanEditError,
  TextEdit,
} from "../wasm-core/typsidian_core.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { describeUnavailable } from "./useElementAtCaret.js";
import { ColumnsField, ConditionsField, OrderField, SelectCount } from "./SelectFields.js";
import { useI18n, type Phrase, type Translate } from "../i18n/index.js";

// Форма параметров элемента Typst (фича 008, макет «Typsidian Вставка» →
// § Параметры блока).
//
// Одна форма на весь каталог и на оба случая — вставить новый элемент и
// поправить готовый. Поля она не знает сама: строит их по схеме элемента,
// которую отдаёт ядро (FR-001a). Решений о языке здесь нет вовсе (принцип I):
// кавычки, скобки, проверку значений, текст «Будет вставлено» и план правки
// делает ядро; форма держит набранное и показывает ответ.

/** Ключ схемы блока выборки в ядре (фича 020, research.md §15). */
export const SELECT_ELEMENT_ID = "call:select";

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
  const { t } = useI18n();
  const editing = mode.kind === "edit";
  const parsed = mode.kind === "edit" ? mode.parsed : null;
  const params = spec.params ?? [];
  const labelPrefix = spec.label?.prefix ?? null;
  const isSelect = spec.id === SELECT_ELEMENT_ID;

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
    // Блок выборки: все пять полей на виду (макет B4), «всех параметров» нет.
    if (isSelect) return { main: all, rest: [] };
    return {
      main: all.filter((def) => def.param.main),
      rest: all.filter((def) => !def.param.main),
    };
  }, [params, showInner, spec.inner, isSelect]);

  // Ключи свойств vault — подсказки полей выборки (макет B4).
  const [propertyKeys, setPropertyKeys] = useState<PropertyKeyInfo[]>([]);
  useEffect(() => {
    if (!isSelect) return;
    let stale = false;
    void client.properties
      .keys()
      .then((keys) => !stale && setPropertyKeys(keys))
      .catch(() => undefined);
    return () => {
      stale = true;
    };
  }, [client, isSelect]);

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
  const [refusal, setRefusal] = useState<Phrase | null>(null);
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
          setRefusal(() => describeRefusal(outcome.error));
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
      isSelect={isSelect}
      keys={propertyKeys}
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
      aria-label={t(editing ? "insert.form.editTitle" : "insert.form.insertTitle", { title })}
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
          {t(editing ? "insert.form.editTitle" : "insert.form.insertTitle", { title })}
        </span>
        <span className="ts-params__tag">
          {spec.syntax === "set" ? `#set ${spec.typst}` : `#${spec.typst}`}
        </span>
        <button type="button" className="ts-btn ts-btn--icon" aria-label={t("common.close")} onClick={onCancel}>
          <Icon name="x" size={12} />
        </button>
      </div>

      <div className="ts-dialog__body ts-params">
        {defs.main.map(renderField)}

        {labelPrefix !== null && (
          <label className="ts-field">
            <span className="ts-field__label">{t("insert.form.label")}</span>
            <div className="ts-labelfield">
              {initialLabel.prefixed && (
                <span className="ts-labelfield__prefix">{labelPrefix}:</span>
              )}
              <input
                className={`ts-field__input${labelError ? " ts-field__input--bad" : ""}`}
                type="text"
                value={labelText}
                placeholder={t("insert.form.noLabel")}
                aria-invalid={labelError !== undefined}
                onChange={(event) => setLabelText(event.target.value)}
              />
            </div>
            {labelError && (
              <span className="ts-field__hint ts-field__hint--bad" role="alert">
                {describeProblem(t, labelError)}
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
              {t("insert.form.allParams")} <span className="ts-params__count">{defs.rest.length}</span>
            </summary>
            {showAll && <div className="ts-params__allbody">{defs.rest.map(renderField)}</div>}
          </details>
        )}

        {refusal && (
          <p className="ts-field__hint ts-field__hint--bad" role="alert">
            {refusal(t)}
          </p>
        )}
        {blockedReason && (
          <p className="ts-field__hint ts-field__hint--bad" role="alert">
            {blockedReason}
          </p>
        )}

        {isSelect && (
          <SelectCount client={client} conditions={fields["where"]?.text ?? "[]"} />
        )}

        <div className="ts-params__output">
          <span className="ts-eyebrow">{editing ? t("insert.form.willReplace") : t("insert.form.willInsert")}</span>
          <pre className="ts-insert__code">{preview}</pre>
        </div>
      </div>

      <div className="ts-dialog__actions ts-params__foot">
        {labelFull && <span className="ts-params__note">{t("insert.form.refNote", { label: labelFull })}</span>}
        <span className="ts-spacer" />
        <button type="button" className="ts-btn" onClick={onCancel}>
          {t("common.cancel")}
        </button>
        <button type="button" className="ts-btn ts-btn--accent" disabled={!canApply} onClick={apply}>
          {editing ? t("insert.form.apply") : t("insert.menu.insert")}
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
  isSelect,
  keys,
  onChange,
}: {
  isSelect: boolean;
  keys: PropertyKeyInfo[];
  def: FieldDef;
  state: FieldState | undefined;
  problem: FieldProblem | undefined;
  autoFocus: boolean;
  onChange: (name: string, text: string, expression: boolean) => void;
}): React.JSX.Element {
  const { param, name } = def;
  const { t } = useI18n();
  const text = state?.text ?? "";
  const expression = state?.expression ?? param.kind.type === "expression";
  const base = baseKind(param.kind);
  const placeholder = param.default ? t("insert.form.default", { value: param.default }) : "";
  const bad = problem !== undefined;
  const inputClass = `ts-field__input${expression ? " ts-field__input--code" : ""}${bad ? " ts-field__input--bad" : ""}`;

  const change = (value: string): void => onChange(name, value, false);
  let control: React.JSX.Element;
  if (base.type === "conditions") {
    control = <ConditionsField value={text || "[]"} keys={keys} onChange={change} />;
  } else if (base.type === "order") {
    control = <OrderField value={text || "null"} keys={keys} onChange={change} />;
  } else if (base.type === "columns") {
    control = <ColumnsField value={text || "[]"} keys={keys} onChange={change} />;
  } else if (!expression && (base.type === "choice" || base.type === "bool")) {
    const options =
      base.type === "bool"
        ? [
            { value: "true", label: t("insert.form.yes") },
            { value: "false", label: t("insert.form.no") },
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
        <option value="">{param.default ? t("insert.form.defaultParen", { value: param.default }) : "—"}</option>
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

  const customLabel = isSelect ? selectFieldLabel(t, name) : null;
  const Wrapper = isCustomKind(base) ? "div" : "label";
  return (
    <Wrapper className="ts-field">
      {/* Подсказка — первый абзац справочника Typst: подпись поля — имя
          параметра, и что он делает, объясняет сам Typst. */}
      <span className="ts-field__label" title={param.docs || undefined}>
        {customLabel ?? param.title}
        {param.required && !param.variadic && <span className="ts-field__req"> *</span>}
      </span>
      {control}
      {expression && param.kind.type !== "expression" && (
        <span className="ts-field__hint">
          {t("insert.form.expression")}
        </span>
      )}
      {bad && (
        <span className="ts-field__hint ts-field__hint--bad" role="alert">
          {describeProblem(t, problem)}
        </span>
      )}
    </Wrapper>
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
  return (
    base.type === "string" ||
    base.type === "content" ||
    isCustomKind(base)
  );
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

export function describeProblem(t: Translate, problem: FieldProblem | undefined): string {
  switch (problem?.kind) {
    case "Required":
      return t("insert.form.problem.required");
    case "Unparsable":
      return t("insert.form.problem.unparsable");
    case "WrongKind":
      return t("insert.form.problem.wrongKind");
    case "NotAnOption":
      return t("insert.form.problem.notAllowed");
    case "LabelTaken":
      return t("insert.form.problem.duplicateLabel");
    case "LabelInvalid":
      return t("insert.form.problem.labelChars");
    default:
      return "";
  }
}

function describeRefusal(error: PlanEditError): Phrase {
  switch (error.kind) {
    case "Stale":
      return (t) => t("insert.form.refusal.stale");
    case "Gone":
      return (t) => t("insert.form.refusal.gone");
    case "Unavailable":
      return (t) => describeUnavailable(t, error.reason);
    case "Invalid":
      return () => "";
    case "Properties":
      return (t) => t("properties.refused");
  }
}

/** Поля выборки, значение которых — JSON структуры (фича 020, research.md §15). */
function isCustomKind(kind: ParamKind): boolean {
  return kind.type === "conditions" || kind.type === "order" || kind.type === "columns";
}

/** Подпись поля блока выборки на языке интерфейса (макет B4). */
function selectFieldLabel(t: Translate, name: string): string | null {
  switch (name) {
    case "where":
      return t("properties.select.field.where");
    case "sort":
      return t("properties.select.field.sort");
    case "columns":
      return t("properties.select.field.columns");
    case "limit":
      return t("properties.select.field.limit");
    case "view":
      return t("properties.select.field.view");
    default:
      return null;
  }
}
