"use client";

import { useEffect, useState } from "react";
import type {
  CompareOp,
  Condition,
  DateRange,
  PropertyKeyInfo,
  SortKey,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { valueFromInput, valueInput } from "../note-panel/properties/conditions.js";
import { isoDate, parseIsoDate } from "../note-panel/properties/format.js";

// Поля формы блока выборки (фича 020, макет B4): «условия», «порядок»,
// «столбцы». Значение каждого — JSON в `FieldValue.plain` (research.md §15):
// запись Typst строит и читает ядро, форма держит только структуру.

/** Встроенные столбцы: имена — как их знает ядро, подписи — на языке интерфейса. */
const BUILTIN_COLUMNS = ["название", "папка", "изменена", "создана"] as const; // i18n-ignore: имена столбцов ядра
/** Столбец, который ядро не хранит ни на одном таргете (research.md §11). */
const UNAVAILABLE_COLUMN = "создана"; // i18n-ignore: имя столбца ядра

const OPS: CompareOp[] = ["Eq", "Ne", "Lt", "Gt", "Le", "Ge"];

type ConditionKind = "tag" | "folder" | "compare" | "exists" | "modified" | "created";

function kindOf(condition: Condition): ConditionKind {
  switch (condition.kind) {
    case "TagHas":
      return "tag";
    case "Folder":
      return "folder";
    case "Compare":
      return "compare";
    case "Exists":
      return "exists";
    case "Modified":
      return "modified";
    case "Created":
      return "created";
  }
}

function blank(kind: ConditionKind): Condition {
  switch (kind) {
    case "tag":
      return { kind: "TagHas", tag: "" };
    case "folder":
      return { kind: "Folder", folder: "" };
    case "compare":
      return { kind: "Compare", key: "", op: "Eq", value: { kind: "Text", value: "" } };
    case "exists":
      return { kind: "Exists", key: "" };
    case "modified":
      return { kind: "Modified", range: { kind: "LastDays", days: 30 } };
    case "created":
      return { kind: "Created", range: { kind: "LastDays", days: 30 } };
  }
}

export function parseJson<T>(text: string, fallback: T): T {
  if (text.trim() === "") return fallback;
  try {
    return JSON.parse(text) as T;
  } catch {
    return fallback;
  }
}

interface FieldProps {
  value: string;
  keys: PropertyKeyInfo[];
  onChange: (value: string) => void;
}

export function ConditionsField({ value, keys, onChange }: FieldProps): React.JSX.Element {
  const { t } = useI18n();
  const conditions = parseJson<Condition[]>(value, []);
  const listId = "ts-select-keys";
  const write = (next: Condition[]): void => onChange(JSON.stringify(next));
  const replace = (index: number, next: Condition): void =>
    write(conditions.map((condition, at) => (at === index ? next : condition)));

  return (
    <div className="ts-select-form__conds">
      <datalist id={listId}>
        {keys.map((info) => (
          <option key={info.key} value={info.key} />
        ))}
      </datalist>
      {conditions.map((condition, index) => (
        <div key={index} className="ts-select-form__cond">
          <select
            className="ts-field__input"
            value={kindOf(condition)}
            aria-label={t("properties.select.conditionKind")}
            onChange={(event) => replace(index, blank(event.target.value as ConditionKind))}
          >
            <option value="tag">{t("properties.cond.tag")}</option>
            <option value="folder">{t("properties.cond.folder")}</option>
            <option value="compare">{t("properties.select.conditionKey")} (=, &lt;, …)</option>
            <option value="exists">{t("properties.cond.exists")}</option>
            <option value="modified">{t("properties.cond.modified")}</option>
            <option value="created" disabled={kindOf(condition) !== "created"}>
              {t("properties.select.createdUnavailable")}
            </option>
          </select>
          <ConditionBody
            condition={condition}
            listId={listId}
            onChange={(next) => replace(index, next)}
          />
          <button
            type="button"
            className="ts-btn ts-btn--icon ts-btn--quiet"
            aria-label={t("properties.select.removeCondition")}
            onClick={() => write(conditions.filter((_, at) => at !== index))}
          >
            <Icon name="x" size={11} />
          </button>
        </div>
      ))}
      <button
        type="button"
        className="ts-btn ts-btn--quiet ts-select-form__add"
        onClick={() => write([...conditions, blank("tag")])}
      >
        {t("properties.select.addCondition")}
      </button>
    </div>
  );
}

function ConditionBody({
  condition,
  listId,
  onChange,
}: {
  condition: Condition;
  listId: string;
  onChange: (next: Condition) => void;
}): React.JSX.Element {
  const { t } = useI18n();
  switch (condition.kind) {
    case "TagHas":
      return (
        <input
          className="ts-field__input ts-select-form__wide"
          value={condition.tag}
          placeholder={t("properties.select.tagName")}
          aria-label={t("properties.select.conditionValue")}
          onChange={(event) => onChange({ kind: "TagHas", tag: event.target.value })}
        />
      );
    case "Folder":
      return (
        <input
          className="ts-field__input ts-select-form__wide"
          value={condition.folder}
          placeholder={t("properties.select.folderName")}
          aria-label={t("properties.select.conditionValue")}
          onChange={(event) => onChange({ kind: "Folder", folder: event.target.value })}
        />
      );
    case "Exists":
      return (
        <input
          className="ts-field__input ts-select-form__wide"
          list={listId}
          value={condition.key}
          placeholder={t("properties.select.conditionKey")}
          aria-label={t("properties.select.conditionKey")}
          onChange={(event) => onChange({ kind: "Exists", key: event.target.value })}
        />
      );
    case "Compare":
      return (
        <span className="ts-select-form__cmp">
          <input
            className="ts-field__input"
            list={listId}
            value={condition.key}
            placeholder={t("properties.select.conditionKey")}
            aria-label={t("properties.select.conditionKey")}
            onChange={(event) => onChange({ ...condition, key: event.target.value })}
          />
          <select
            className="ts-field__input"
            value={condition.op}
            aria-label={t("properties.select.conditionOp")}
            onChange={(event) => onChange({ ...condition, op: event.target.value as CompareOp })}
          >
            {OPS.map((op) => (
              <option key={op} value={op}>
                {t(`properties.op.${op}`)}
              </option>
            ))}
          </select>
          <input
            className="ts-field__input"
            value={valueInput(condition.value)}
            aria-label={t("properties.select.conditionValue")}
            onChange={(event) =>
              onChange({ ...condition, value: valueFromInput(event.target.value) })
            }
          />
        </span>
      );
    case "Modified":
    case "Created": {
      const make = (range: DateRange): Condition =>
        condition.kind === "Modified"
          ? { kind: "Modified", range }
          : { kind: "Created", range };
      const range = condition.range;
      return (
        <span className="ts-select-form__cmp">
          <select
            className="ts-field__input"
            value={range.kind}
            aria-label={t("properties.select.rangeKind")}
            onChange={(event) =>
              onChange(
                make(
                  event.target.value === "LastDays"
                    ? { kind: "LastDays", days: 30 }
                    : {
                        kind: "Between",
                        from: { year: 2026, month: 1, day: 1 },
                        to: { year: 2026, month: 12, day: 31 },
                      },
                ),
              )
            }
          >
            <option value="LastDays">{t("properties.select.range.days")}</option>
            <option value="Between">{t("properties.select.range.between")}</option>
          </select>
          {range.kind === "LastDays" ? (
            <input
              className="ts-field__input"
              type="number"
              min={1}
              value={range.days}
              aria-label={t("properties.select.days")}
              onChange={(event) =>
                onChange(make({ kind: "LastDays", days: Math.max(1, Number(event.target.value) || 1) }))
              }
            />
          ) : (
            <>
              <input
                className="ts-field__input"
                type="date"
                value={isoDate(range.from)}
                aria-label={t("properties.select.from")}
                onChange={(event) => {
                  const from = parseIsoDate(event.target.value);
                  if (from !== null) onChange(make({ ...range, from }));
                }}
              />
              <input
                className="ts-field__input"
                type="date"
                value={isoDate(range.to)}
                aria-label={t("properties.select.to")}
                onChange={(event) => {
                  const to = parseIsoDate(event.target.value);
                  if (to !== null) onChange(make({ ...range, to }));
                }}
              />
            </>
          )}
        </span>
      );
    }
  }
}

export function OrderField({ value, keys, onChange }: FieldProps): React.JSX.Element {
  const { t } = useI18n();
  const sort = parseJson<SortKey | null>(value, null);
  const listId = "ts-select-order-keys";
  return (
    <div className="ts-select-form__order">
      <datalist id={listId}>
        {[...BUILTIN_COLUMNS.filter((name) => name !== UNAVAILABLE_COLUMN), ...keys.map((k) => k.key)].map(
          (name) => (
            <option key={name} value={name} />
          ),
        )}
      </datalist>
      <input
        className="ts-field__input"
        list={listId}
        value={sort?.key ?? ""}
        placeholder={t("properties.select.orderNone")}
        aria-label={t("properties.select.orderKey")}
        onChange={(event) =>
          onChange(
            event.target.value === ""
              ? "null"
              : JSON.stringify({ key: event.target.value, descending: sort?.descending ?? false }),
          )
        }
      />
      <select
        className="ts-field__input"
        value={sort?.descending === true ? "desc" : "asc"}
        disabled={sort === null}
        aria-label={t("properties.select.field.sort")}
        onChange={(event) =>
          sort !== null &&
          onChange(JSON.stringify({ key: sort.key, descending: event.target.value === "desc" }))
        }
      >
        <option value="asc">{t("properties.select.ascending")}</option>
        <option value="desc">{t("properties.select.descending")}</option>
      </select>
    </div>
  );
}

export function ColumnsField({ value, keys, onChange }: FieldProps): React.JSX.Element {
  const { t } = useI18n();
  const columns = parseJson<string[]>(value, []);
  const [adding, setAdding] = useState("");
  const listId = "ts-select-columns";
  const labelOf = (name: string): string => {
    switch (name) {
      case "название": // i18n-ignore: имя столбца ядра
        return t("properties.select.column.name");
      case "папка": // i18n-ignore: имя столбца ядра
        return t("properties.select.column.folder");
      case "изменена": // i18n-ignore: имя столбца ядра
        return t("properties.select.column.modified");
      case "создана": // i18n-ignore: имя столбца ядра
        return t("properties.select.column.created");
      default:
        return name;
    }
  };
  const write = (next: string[]): void => onChange(JSON.stringify(next));
  const add = (): void => {
    const name = adding.trim();
    setAdding("");
    if (name !== "" && !columns.includes(name)) write([...columns, name]);
  };
  return (
    <div className="ts-select-form__cols">
      <datalist id={listId}>
        {[...BUILTIN_COLUMNS.filter((name) => name !== UNAVAILABLE_COLUMN), ...keys.map((k) => k.key)]
          .filter((name) => !columns.includes(name))
          .map((name) => (
            <option key={name} value={name} />
          ))}
      </datalist>
      {columns.map((name) => (
        <span key={name} className="ts-props__chip">
          <span className="ts-props__chipbtn">{labelOf(name)}</span>
          <button
            type="button"
            className="ts-props__chipx"
            aria-label={t("properties.select.removeColumn", { column: labelOf(name) })}
            onClick={() => write(columns.filter((other) => other !== name))}
          >
            <Icon name="x" size={9} />
          </button>
        </span>
      ))}
      <input
        className="ts-field__input ts-props__input--chip"
        list={listId}
        value={adding}
        placeholder={t("properties.select.addColumn")}
        aria-label={t("properties.select.addColumn")}
        onChange={(event) => setAdding(event.target.value)}
        onBlur={add}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            event.stopPropagation();
            add();
          }
        }}
      />
    </div>
  );
}

/** «Сейчас подходят N заметок» — отбор по условиям формы (B4). */
export function SelectCount({
  client,
  conditions,
}: {
  client: VaultClient;
  conditions: string;
}): React.JSX.Element {
  const { t } = useI18n();
  const [count, setCount] = useState<number | "failed" | null>(null);
  useEffect(() => {
    let stale = false;
    const timer = window.setTimeout(() => {
      void client.properties
        .select({
          conditions: parseJson<Condition[]>(conditions, []),
          columns: [],
          limit: 1,
          view: "Table",
        })
        .then((result) => !stale && setCount(result.total))
        .catch(() => !stale && setCount("failed"));
    }, 200);
    return () => {
      stale = true;
      window.clearTimeout(timer);
    };
  }, [client, conditions]);
  if (count === null) return <span />;
  return (
    <span className="ts-select-form__count" role="status">
      {count === "failed"
        ? t("properties.select.countFailed")
        : t("properties.select.count", { count })}
    </span>
  );
}
