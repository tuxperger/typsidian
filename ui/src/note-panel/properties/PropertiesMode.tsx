"use client";

import { useState } from "react";
import type {
  Note,
  NoteProperties,
  PropertiesProblem,
  PropertyEdit,
  PropertyValue,
  ValueKind,
} from "../../wasm-core/typsidian_core.js";
import { useI18n, type Translate } from "../../i18n/index.js";
import { defaultValue, KIND_GLYPH } from "./format.js";
import { PropertyRow } from "./PropertyRow.js";
import type { RevealPlace } from "../links/LinksMode.js";

// Режим «Свойства» колонки заметки (фича 020, макет B1/B2,
// contracts/frontend-properties.md). Что в записи и можно ли её править,
// говорит ядро (`NoteProperties.editable`); форма только рисует ответ.

export interface PropertiesModeProps {
  /** Открыта ли заметка. */
  hasTarget: boolean;
  note: Note | null;
  /** `null` — запись ещё не прочитана. */
  properties: NoteProperties | null;
  /** Vault только для чтения: форма выключена. */
  readOnly: boolean;
  /** Библиотека выборки правилась вручную и оставлена как есть (research.md §10). */
  libraryCustomized: boolean;
  /** Применить правку записи к заметке; `Refused` объясняется пользователю. */
  onEdit: (change: PropertyEdit) => Promise<PropertiesEditResult>;
  onReveal: (place: RevealPlace) => void;
  /** Клик по тегу: поиск с условием «тег» (B3). */
  onPickTag: (tag: string) => void;
}

/** Итог правки для панели: удалась ли и, если нет, почему. */
export type PropertiesEditResult =
  | { kind: "applied" }
  | { kind: "refused"; problem: PropertiesProblem | null };

const NEW_KINDS: Exclude<ValueKind, "Unknown">[] = ["Text", "Number", "Date", "Flag", "List"];

export function PropertiesMode({
  hasTarget,
  note,
  properties,
  readOnly,
  libraryCustomized,
  onEdit,
  onReveal,
  onPickTag,
}: PropertiesModeProps): React.JSX.Element {
  const { t, formatDate } = useI18n();
  const [refusal, setRefusal] = useState<PropertiesProblem | null | "other">(null);

  if (!hasTarget || note === null) {
    return <p className="ts-links__empty">{t("properties.openNote")}</p>;
  }
  if (properties === null) {
    return <p className="ts-links__empty">{t("properties.loading")}</p>;
  }

  const disabled = readOnly || !properties.editable;
  const tagsProperty = properties.properties.find(
    (property) => property.key === "tags" && property.value.kind === "List",
  );
  const others = properties.properties.filter((property) => property !== tagsProperty);
  const problem = properties.problem;

  const edit = (change: PropertyEdit): void => {
    void (async () => {
      const result = await onEdit(change);
      setRefusal(result.kind === "applied" ? null : (result.problem ?? "other"));
    })();
  };

  const folder = note.path.includes("/") ? `${note.path.slice(0, note.path.lastIndexOf("/"))}/` : t("properties.rootFolder");

  return (
    <div className="ts-props">
      {!properties.editable && (
        <div className="ts-props__warn" role="status">
          <b>{t("properties.readOnly.title")}</b>
          <span>
            {problem !== undefined ? problemPhrase(t, problem) : t("properties.refused")}{" "}
            {t("properties.readOnly.tail")}
          </span>
          {properties.record !== undefined && (
            <button
              type="button"
              className="ts-btn ts-btn--outline"
              onClick={() =>
                properties.record !== undefined &&
                onReveal({
                  path: note.path,
                  line: properties.record.line,
                  column: properties.record.column,
                })
              }
            >
              {t("properties.readOnly.goto")}
            </button>
          )}
        </div>
      )}
      {libraryCustomized && (
        <p className="ts-props__warn ts-props__warn--plain" role="status">
          {t("properties.libraryCustomized")}
        </p>
      )}

      <div className="ts-props__list" role="list" aria-label={t("properties.listLabel")}>
        {/* Теги — первой строкой, даже когда их ещё нет (макет B1). */}
        <PropertyRow
          propertyKey="tags"
          value={tagsProperty?.value ?? { kind: "List", values: [] }}
          disabled={disabled}
          onEdit={(change) =>
            // Последний тег убран — запись `tags` удаляется, а не остаётся пустой.
            edit(
              change.kind === "Set" && change.value.kind === "List" && change.value.values.length === 0
                ? { kind: "Remove", key: "tags" }
                : change,
            )
          }
          onPickTag={onPickTag}
        />
        {others.map((property) => (
          <PropertyRow
            key={property.key}
            propertyKey={property.key}
            value={property.value}
            disabled={disabled}
            onEdit={edit}
            onPickTag={onPickTag}
          />
        ))}
      </div>

      {!disabled && (
        <AddProperty
          existing={properties.properties.map((property) => property.key)}
          onAdd={(key, value) => edit({ kind: "Set", key, value })}
        />
      )}

      {refusal !== null && (
        <p className="ts-field__hint ts-field__hint--bad" role="alert">
          {refusal === "other" ? t("properties.refused") : problemPhrase(t, refusal)}
        </p>
      )}

      <div className="ts-eyebrow ts-props__sub">{t("properties.fileInfo")}</div>
      <div className="ts-props__list" role="list">
        <BuiltinRow glyph={KIND_GLYPH.Text} name={t("properties.folder")} value={folder} />
        <BuiltinRow
          glyph={KIND_GLYPH.Date}
          name={t("properties.modified")}
          value={
            note.stamp.mtime_millis > 0
              ? formatDate(new Date(note.stamp.mtime_millis))
              : t("properties.createdUnknown")
          }
        />
        {/* «Создана» ядро не хранит ни на одном таргете (research.md §11). */}
        <BuiltinRow glyph={KIND_GLYPH.Date} name={t("properties.created")} value={t("properties.createdUnknown")} muted />
      </div>
      <p className="ts-props__foot">{t("properties.status.hidden")}</p>
    </div>
  );
}

function BuiltinRow({
  glyph,
  name,
  value,
  muted = false,
}: {
  glyph: string;
  name: string;
  value: string;
  muted?: boolean;
}): React.JSX.Element {
  return (
    <div className="ts-props__row ts-props__row--builtin" role="listitem">
      <span className="ts-props__glyph" aria-hidden="true">
        {glyph}
      </span>
      <span className="ts-props__key">{name}</span>
      <span className={muted ? "ts-props__value ts-props__value--muted" : "ts-props__value"}>
        {value}
      </span>
    </div>
  );
}

/** «+ Добавить свойство»: имя и вид, значение потом правится в строке. */
function AddProperty({
  existing,
  onAdd,
}: {
  existing: string[];
  onAdd: (key: string, value: PropertyValue) => void;
}): React.JSX.Element {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [key, setKey] = useState("");
  const [kind, setKind] = useState<Exclude<ValueKind, "Unknown">>("Text");
  const trimmed = key.trim();
  const error =
    trimmed === "" ? null : existing.includes(trimmed) ? t("properties.add.exists") : null;

  const close = (): void => {
    setOpen(false);
    setKey("");
  };
  const submit = (): void => {
    if (trimmed === "" || error !== null) return;
    onAdd(trimmed, defaultValue(kind));
    close();
  };

  if (!open) {
    return (
      <button type="button" className="ts-btn ts-btn--quiet ts-props__add" onClick={() => setOpen(true)}>
        {t("properties.add")}
      </button>
    );
  }
  return (
    <form
      className="ts-props__newform"
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.stopPropagation();
          close();
        }
      }}
    >
      <input
        className={`ts-field__input ts-props__input${error !== null ? " ts-field__input--bad" : ""}`}
        type="text"
        autoFocus
        value={key}
        placeholder={t("properties.add.keyPlaceholder")}
        aria-label={t("properties.add.key")}
        aria-invalid={error !== null}
        onChange={(event) => setKey(event.target.value)}
      />
      <select
        className="ts-field__input ts-props__input"
        value={kind}
        aria-label={t("properties.add.kind")}
        onChange={(event) => setKind(event.target.value as Exclude<ValueKind, "Unknown">)}
      >
        {NEW_KINDS.map((option) => (
          <option key={option} value={option}>
            {t(`properties.kind.${option}`)}
          </option>
        ))}
      </select>
      {error !== null && (
        <span className="ts-field__hint ts-field__hint--bad" role="alert">
          {error}
        </span>
      )}
      <span className="ts-props__newactions">
        <button type="button" className="ts-btn ts-btn--outline" onClick={close}>
          {t("properties.add.cancel")}
        </button>
        <button type="submit" className="ts-btn ts-btn--accent" disabled={trimmed === "" || error !== null}>
          {t("properties.add.confirm")}
        </button>
      </span>
    </form>
  );
}

/** Причина «запись только для чтения» словами (B2). */
export function problemPhrase(t: Translate, problem: PropertiesProblem): string {
  switch (problem.kind) {
    case "Malformed":
      return t("properties.problem.Malformed");
    case "SecondRecord":
      return t("properties.problem.SecondRecord");
    case "NotADictionary":
      return t("properties.problem.NotADictionary");
    case "Spread":
      return t("properties.problem.Spread");
    case "DuplicateKey":
      return t("properties.problem.DuplicateKey", { key: problem.key });
    case "NotLiteral":
      return t("properties.problem.NotLiteral", { key: problem.key });
  }
}
