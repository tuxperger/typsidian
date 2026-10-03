"use client";

import { useEffect, useRef, useState } from "react";
import type {
  FolderProblem,
  NameCheck,
  NameProblem,
  SaveTemplateError,
  Template,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { describeFolderProblem } from "./TemplateGallery.js";
import { useI18n, type Translate } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

// «Сохранить заметку как заготовку» (T033, US2).
//
// SC-006: сохранение занимает не больше трёх действий — открыть диалог, ввести
// имя, подтвердить. Поэтому здесь нет ни выбора папки (папка одна и фиксирована
// — research.md §2), ни выбора вида заготовки (видов не существует — FR-019).
//
// # Правило коллизии живёт в ядре, а не здесь
//
// Диалог показывает вопрос и передаёт ответ. Проверка «если файл есть —
// спросить» на стороне UI дала бы гонку: файл мог появиться между проверкой и
// записью. Поэтому `saveAsTemplate` без подтверждения откажет сам, даже если
// проверка ниже пропустила коллизию (research.md §8).

export interface SaveAsTemplateDialogProps {
  client: VaultClient;
  /** Что сохраняем — текущий буфер редактора, а не файл с диска. */
  content: string;
  /** Имя открытой заметки: из него берётся имя заготовки по умолчанию. */
  suggestedName: string;
  onSaved: (template: Template) => void;
  onCancel: () => void;
}

export function SaveAsTemplateDialog({
  client,
  content,
  suggestedName,
  onSaved,
  onCancel,
}: SaveAsTemplateDialogProps): React.JSX.Element {
  const { t } = useI18n();
  const [name, setName] = useState(() => stemOf(suggestedName));
  const [check, setCheck] = useState<NameCheck | null>(null);
  const [failure, setFailure] = useState<SaveTemplateError | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // Пока идёт запись, отменять нечего: файл уже пишется.
  useDismissOnEscape(isSaving ? null : onCancel);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  // Проверка имени идёт в ядро на каждое изменение: она ничего не меняет на
  // диске (FR-029) и стоит одного взгляда в индекс.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const result = await client.checkTemplateName(name);
      if (!cancelled) {
        setCheck(result);
        setFailure(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, name]);

  const taken = check?.check === "NameTaken";
  const invalid = check?.check === "Invalid" ? check.reason : null;

  const save = async (confirmOverwrite: boolean): Promise<void> => {
    setIsSaving(true);
    setFailure(null);
    try {
      const saved = await client.saveAsTemplate(name, content, confirmOverwrite);
      onSaved(saved);
    } catch (cause) {
      setFailure(asSaveError(cause));
    } finally {
      setIsSaving(false);
    }
  };

  // `.ts-dialog` рисует затемнение сам (тень на весь вьюпорт), поэтому обёртки
  return (
      <div
        className="ts-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t("templates.save.title")}
      >
        <div className="ts-dialog__head">
          <span className="ts-eyebrow">{t("templates.save.title")}</span>
        </div>

        <div className="ts-dialog__body">
          <label className="ts-field">
            <span className="ts-field__label">{t("templates.save.name")}</span>
            <input
              ref={inputRef}
              className="ts-field__input"
              value={name}
              onChange={(event) => setName(event.target.value)}
              onKeyDown={(event) => {
                // Enter сохраняет, но только когда сохранять можно: на занятом
                // имени он не должен молча перезаписать чужой файл (FR-028).
                if (event.key === "Enter" && check?.check === "Ok" && !isSaving) {
                  event.preventDefault();
                  void save(false);
                }
              }}
            />
          </label>

          <p className="ts-dialog__hint">
            {t("templates.save.hintBefore")}
            <code>templates/</code>
            {t("templates.save.hintAfter")}
          </p>

          {invalid !== null && (
            <p className="ts-dialog__error" role="alert">
              {describeNameProblem(t, invalid)}
            </p>
          )}

          {taken && (
            <p className="ts-dialog__warn" role="alert">
              {t("templates.save.taken")}
            </p>
          )}

          {failure !== null && (
            <p className="ts-dialog__error" role="alert">
              {describeSaveError(t, failure)}
            </p>
          )}
        </div>

        <div className="ts-dialog__foot">
          <button type="button" className="ts-btn" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <span className="ts-spacer" />
          {taken ? (
            <button
              type="button"
              className="ts-btn ts-btn--danger"
              disabled={isSaving}
              onClick={() => void save(true)}
            >
              {t("templates.save.overwrite")}
            </button>
          ) : (
            <button
              type="button"
              className="ts-btn ts-btn--accent"
              disabled={isSaving || check?.check !== "Ok"}
              onClick={() => void save(false)}
            >
              {t("common.save")}
            </button>
          )}
        </div>
      </div>
  );
}

/** Имя заметки без расширения — заготовку называют так же, но своим именем. */
function stemOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/** Почему имя не годится (FR-029). */
export function describeNameProblem(t: Translate, reason: NameProblem): string {
  switch (reason) {
    case "Empty":
      return t("templates.save.empty");
    case "IllegalCharacters":
      return t("templates.save.illegal");
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}

/** Почему сохранение отклонено — причина, а не общий отказ (FR-030). */
export function describeSaveError(t: Translate, error: SaveTemplateError): string {
  switch (error.kind) {
    case "NameTaken":
      return t("templates.save.nameTaken");
    case "InvalidName":
      return describeNameProblem(t, error.reason);
    case "FolderNotWritable":
      return t("templates.save.folderNotWritable", {
        reason: describeFolderProblem(t, error.reason),
      });
    case "Io":
      return t("templates.save.io");
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

/** Сужение `unknown` до `SaveTemplateError` без `any` (принцип IV). */
function asSaveError(cause: unknown): SaveTemplateError {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const value = cause as { kind: unknown; reason?: unknown };
    if (value.kind === "NameTaken") return { kind: "NameTaken" };
    if (value.kind === "Io") return { kind: "Io" };
    if (value.kind === "InvalidName" && isNameProblem(value.reason)) {
      return { kind: "InvalidName", reason: value.reason };
    }
    if (value.kind === "FolderNotWritable" && isFolderProblem(value.reason)) {
      return { kind: "FolderNotWritable", reason: value.reason };
    }
  }
  return { kind: "Io" };
}

function isNameProblem(value: unknown): value is NameProblem {
  return value === "Empty" || value === "IllegalCharacters";
}

function isFolderProblem(value: unknown): value is FolderProblem {
  return (
    value === "AccessDenied" ||
    value === "NotADirectory" ||
    value === "VaultReadOnly" ||
    value === "Io"
  );
}
