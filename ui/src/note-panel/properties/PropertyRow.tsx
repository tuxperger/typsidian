"use client";

import { useEffect, useState } from "react";
import type { PropertyEdit, PropertyValue } from "../../wasm-core/typsidian_core.js";
import { useI18n } from "../../i18n/index.js";
import { Icon } from "../../icons/Icons.js";
import { isoDate, KIND_GLYPH, parseIsoDate, TAGS_KEY } from "./format.js";

// Строка свойства (фича 020, макет B1/B2): значок вида, ключ, редактор значения
// по виду. Строка ничего не знает о записи в тексте: она отдаёт наверх
// `PropertyEdit`, а план правки строит ядро.

export interface PropertyRowProps {
  propertyKey: string;
  value: PropertyValue;
  /** Запись неоднозначна или vault только для чтения: поля только показывают. */
  disabled: boolean;
  onEdit: (change: PropertyEdit) => void;
  /** Клик по тегу открывает поиск с условием «тег» (B3). */
  onPickTag: (tag: string) => void;
}

export function PropertyRow({
  propertyKey,
  value,
  disabled,
  onEdit,
  onPickTag,
}: PropertyRowProps): React.JSX.Element {
  const { t } = useI18n();
  const isTags = propertyKey === TAGS_KEY && value.kind === "List";
  const label = propertyKey === TAGS_KEY ? t("properties.tags") : propertyKey;
  const set = (next: PropertyValue): void => onEdit({ kind: "Set", key: propertyKey, value: next });

  return (
    <div className="ts-props__row" role="listitem">
      <span className="ts-props__glyph" aria-hidden="true" title={t(`properties.kind.${value.kind}`)}>
        {KIND_GLYPH[value.kind]}
      </span>
      <span className="ts-props__key" title={propertyKey}>
        {label}
      </span>
      <span className="ts-props__value">
        <ValueEditor
          propertyKey={propertyKey}
          value={value}
          disabled={disabled}
          isTags={isTags}
          set={set}
          onPickTag={onPickTag}
        />
      </span>
      {!disabled && (
        <button
          type="button"
          className="ts-btn ts-btn--icon ts-btn--quiet ts-props__remove"
          aria-label={t("properties.remove", { key: label })}
          onClick={() => onEdit({ kind: "Remove", key: propertyKey })}
        >
          <Icon name="x" size={11} />
        </button>
      )}
    </div>
  );
}

interface ValueEditorProps {
  propertyKey: string;
  value: PropertyValue;
  disabled: boolean;
  isTags: boolean;
  set: (value: PropertyValue) => void;
  onPickTag: (tag: string) => void;
}

function ValueEditor({
  propertyKey,
  value,
  disabled,
  isTags,
  set,
  onPickTag,
}: ValueEditorProps): React.JSX.Element {
  const { t } = useI18n();
  switch (value.kind) {
    case "Text":
      return (
        <DraftField
          label={t("properties.valueLabel", { key: propertyKey })}
          shown={value.value}
          disabled={disabled}
          parse={(text) => ({ kind: "Text", value: text })}
          onCommit={set}
        />
      );
    case "Number":
      return (
        <DraftField
          label={t("properties.valueLabel", { key: propertyKey })}
          shown={String(value.value)}
          disabled={disabled}
          numeric
          parse={(text) => {
            const number = Number(text.trim().replace(",", "."));
            return text.trim() === "" || !Number.isFinite(number)
              ? null
              : { kind: "Number", value: number };
          }}
          onCommit={set}
        />
      );
    case "Date":
      return (
        <input
          className="ts-field__input ts-props__input"
          type="date"
          value={isoDate(value)}
          disabled={disabled}
          aria-label={t("properties.valueLabel", { key: propertyKey })}
          onChange={(event) => {
            const parsed = parseIsoDate(event.target.value);
            if (parsed !== null) set({ kind: "Date", ...parsed });
          }}
        />
      );
    case "Flag":
      return (
        <input
          className="ts-props__check"
          type="checkbox"
          checked={value.value}
          disabled={disabled}
          aria-label={t("properties.valueLabel", { key: propertyKey })}
          onChange={(event) => set({ kind: "Flag", value: event.target.checked })}
        />
      );
    case "List":
      return (
        <ChipList
          propertyKey={propertyKey}
          values={value.values}
          disabled={disabled}
          isTags={isTags}
          onChange={(values) => set({ kind: "List", values })}
          onPickTag={onPickTag}
        />
      );
    case "Unknown":
      return (
        <span className="ts-props__unknown" title={value.source}>
          <span className="ts-props__code">{value.source}</span>
          <span className="ts-props__hint">{t("properties.notDefined")}</span>
        </span>
      );
  }
}

/**
 * Поле с черновиком: набранное уходит в запись по Enter или уходу фокуса, а не
 * на каждый символ — иначе каждое нажатие было бы правкой файла и шагом отмены.
 */
function DraftField({
  label,
  shown,
  disabled,
  numeric = false,
  parse,
  onCommit,
}: {
  label: string;
  shown: string;
  disabled: boolean;
  numeric?: boolean;
  parse: (text: string) => PropertyValue | null;
  onCommit: (value: PropertyValue) => void;
}): React.JSX.Element {
  const { t } = useI18n();
  const [draft, setDraft] = useState(shown);
  // Значение в записи сменилось снаружи (правка в тексте): черновик догоняет.
  useEffect(() => setDraft(shown), [shown]);
  const parsed = parse(draft);
  const bad = parsed === null;

  const commit = (): void => {
    if (parsed === null) {
      setDraft(shown);
      return;
    }
    if (draft !== shown) onCommit(parsed);
  };

  return (
    <>
      <input
        className={`ts-field__input ts-props__input${bad ? " ts-field__input--bad" : ""}`}
        type="text"
        inputMode={numeric ? "decimal" : "text"}
        value={draft}
        disabled={disabled}
        aria-label={label}
        aria-invalid={bad}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            commit();
          } else if (event.key === "Escape" && draft !== shown) {
            event.stopPropagation();
            setDraft(shown);
          }
        }}
      />
      {bad && (
        <span className="ts-field__hint ts-field__hint--bad" role="alert">
          {t("properties.badNumber")}
        </span>
      )}
    </>
  );
}

/** Список значений фишками с «+» — теги и любые списки (макет B1). */
function ChipList({
  propertyKey,
  values,
  disabled,
  isTags,
  onChange,
  onPickTag,
}: {
  propertyKey: string;
  values: string[];
  disabled: boolean;
  isTags: boolean;
  onChange: (values: string[]) => void;
  onPickTag: (tag: string) => void;
}): React.JSX.Element {
  const { t } = useI18n();
  const [adding, setAdding] = useState(false);
  const [draft, setDraft] = useState("");

  const add = (): void => {
    const item = draft.trim().replace(/^#/, "");
    setDraft("");
    setAdding(false);
    if (item !== "" && !values.includes(item)) onChange([...values, item]);
  };

  return (
    <>
      {values.map((item) => (
        <span key={item} className="ts-props__chip">
          {isTags ? (
            <button
              type="button"
              className="ts-props__chipbtn"
              title={t("properties.tagPick", { tag: item })}
              onClick={() => onPickTag(item)}
            >
              {item}
            </button>
          ) : (
            <span className="ts-props__chipbtn">{item}</span>
          )}
          {!disabled && (
            <button
              type="button"
              className="ts-props__chipx"
              aria-label={
                isTags
                  ? t("properties.tagRemove", { tag: item })
                  : t("properties.itemRemove", { item })
              }
              onClick={() => onChange(values.filter((other) => other !== item))}
            >
              <Icon name="x" size={9} />
            </button>
          )}
        </span>
      ))}
      {!disabled &&
        (adding ? (
          <input
            className="ts-field__input ts-props__input ts-props__input--chip"
            type="text"
            autoFocus
            value={draft}
            placeholder={t("properties.tagPlaceholder")}
            aria-label={isTags ? t("properties.addTagLabel") : t("properties.addItemLabel", { key: propertyKey })}
            onChange={(event) => setDraft(event.target.value)}
            onBlur={add}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                add();
              } else if (event.key === "Escape") {
                event.stopPropagation();
                setDraft("");
                setAdding(false);
              }
            }}
          />
        ) : (
          <button
            type="button"
            className="ts-props__chip ts-props__chip--add"
            aria-label={
              isTags ? t("properties.addTagLabel") : t("properties.addItemLabel", { key: propertyKey })
            }
            onClick={() => setAdding(true)}
          >
            {isTags ? t("properties.addTag") : t("properties.addItem")}
          </button>
        ))}
    </>
  );
}
