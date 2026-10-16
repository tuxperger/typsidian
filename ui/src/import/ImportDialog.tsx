"use client";

import { useEffect, useRef, useState } from "react";
import type { OnConflict } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import type { ImportControl } from "./useImport.js";

// Диалог импорта из Obsidian (фича 021; макет — артборды C1, C4, C5).
//
// Что нашлось в папке и можно ли продолжить прежний импорт, считает ядро
// (`ImportSurvey`). Здесь выбор папок и режимов и кнопки.

export interface ImportDialogProps {
  control: ImportControl;
}

export function ImportDialog({ control }: ImportDialogProps): React.JSX.Element {
  const { t, formatNumber, formatDate } = useI18n();
  const { setup, available } = control;
  const { survey } = setup;
  useDismissOnEscape(control.closeDialog);

  const resume = survey?.resume ?? null;
  const stage = !available ? "unavailable" : resume !== null ? "resume" : "setup";

  // Фокус — на главное действие экрана: выбор папки, когда её ещё нет, и
  // «Закрыть» там, где делать больше нечего.
  const firstRef = useRef<HTMLButtonElement>(null);
  const focusKey = `${stage}:${setup.source === null}`;
  useEffect(() => {
    firstRef.current?.focus();
  }, [focusKey]);

  const [editingDestination, setEditingDestination] = useState(false);

  const title = t("import.title");

  if (stage === "unavailable") {
    return (
      <div className="ts-dialog ts-import" role="dialog" aria-modal="true" aria-label={title}>
        <div className="ts-dialog__head">
          <span className="ts-eyebrow">{title}</span>
        </div>
        <div className="ts-dialog__body">
          <p className="ts-import__note ts-import__note--warn" role="status">
            {t("import.unavailable")}
          </p>
        </div>
        <div className="ts-dialog__foot">
          <span className="ts-spacer" />
          <button type="button" ref={firstRef} className="ts-btn" onClick={control.closeDialog}>
            {t("common.close")}
          </button>
        </div>
      </div>
    );
  }

  const sourceFacts = (
    <div className="ts-import__facts" aria-live="polite">
      {setup.surveying && survey === null ? (
        <span>{t("import.survey.reading")}</span>
      ) : survey !== null ? (
        <>
          <span>
            <b>{formatNumber(survey.notes)}</b> {t("import.survey.notes", { count: survey.notes })}
          </span>
          <span>
            <b>{formatNumber(survey.images)}</b>{" "}
            {t("import.survey.images", { count: survey.images })}
          </span>
          <span>
            <b>{formatNumber(survey.other)}</b> {t("import.survey.other", { count: survey.other })}
          </span>
          <span>{t("import.survey.excluded")}</span>
        </>
      ) : null}
    </div>
  );

  if (stage === "resume" && resume !== null) {
    const left = Math.max(resume.total - resume.done, 0);
    return (
      <div className="ts-dialog ts-import" role="dialog" aria-modal="true" aria-label={title}>
        <div className="ts-dialog__head">
          <span className="ts-eyebrow">{title}</span>
        </div>
        <div className="ts-dialog__body">
          <p className="ts-import__note">
            {t("import.resume.intro", {
              when: formatDate(new Date(resume.at)),
              done: resume.done,
              total: resume.total,
            })}
          </p>
          <p className="ts-dialog__hint">
            {t("import.resume.hint", {
              done: resume.done,
              left,
            })}
          </p>
        </div>
        <div className="ts-dialog__foot">
          <button type="button" className="ts-btn" onClick={control.closeDialog}>
            {t("common.cancel")}
          </button>
          <button type="button" className="ts-btn" onClick={control.chooseSource}>
            {t("import.resume.fresh")}
          </button>
          <span className="ts-spacer" />
          <button
            type="button"
            ref={firstRef}
            className="ts-btn ts-btn--accent"
            onClick={control.start}
          >
            {t("import.resume.start", { count: left })}
          </button>
        </div>
      </div>
    );
  }

  const conflicts: { value: OnConflict; label: string; hint: string }[] = [
    { value: "Skip", label: t("import.conflict.skip"), hint: t("import.conflict.skipHint") },
    { value: "Beside", label: t("import.conflict.beside"), hint: t("import.conflict.besideHint") },
  ];
  const canStart =
    setup.source !== null && survey !== null && !setup.surveying && survey.notes > 0;
  const destination = setup.destination.trim().replace(/^\/+|\/+$/g, "");

  return (
    <div className="ts-dialog ts-import" role="dialog" aria-modal="true" aria-label={title}>
      <div className="ts-dialog__head">
        <span className="ts-eyebrow">{title}</span>
      </div>

      <div className="ts-dialog__body">
        <div className="ts-field">
          <span className="ts-field__label">{t("import.source.label")}</span>
          <div className="ts-import__row">
            <span className="ts-import__path" title={setup.source?.label}>
              {setup.source?.label ?? t("import.source.none")}
            </span>
            <button
              type="button"
              ref={setup.source === null ? firstRef : undefined}
              className="ts-btn ts-btn--outline"
              onClick={control.chooseSource}
            >
              {setup.source === null ? t("import.source.choose") : t("import.source.again")}
            </button>
          </div>
          {sourceFacts}
        </div>

        <div className="ts-field">
          <span className="ts-field__label">{t("import.dest.label")}</span>
          <div className="ts-import__row">
            {editingDestination ? (
              <input
                className="ts-field__input ts-import__input"
                value={setup.destination}
                aria-label={t("import.dest.input")}
                autoComplete="off"
                spellCheck={false}
                autoFocus
                onChange={(event) => control.setDestination(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    setEditingDestination(false);
                  }
                }}
              />
            ) : (
              <span className="ts-import__path">
                {destination === "" ? t("import.dest.root") : destination.split("/").join(" / ")}
              </span>
            )}
            <button
              type="button"
              className="ts-btn ts-btn--outline"
              onClick={() => setEditingDestination((editing) => !editing)}
            >
              {editingDestination ? t("import.dest.done") : t("import.dest.change")}
            </button>
          </div>
          <p className="ts-dialog__hint">{t("import.dest.hint")}</p>
        </div>

        <fieldset className="ts-import__choices">
          <legend className="ts-field__label">{t("import.conflict.label")}</legend>
          {conflicts.map((option) => (
            <label key={option.value} className="ts-import__choice">
              <input
                type="radio"
                name="import-conflict"
                checked={setup.onConflict === option.value}
                onChange={() => control.setOnConflict(option.value)}
              />
              <span>
                {option.label}
                <small>{option.hint}</small>
              </span>
            </label>
          ))}
        </fieldset>

        <label className="ts-import__choice">
          <input
            type="checkbox"
            checked={setup.copyOther}
            onChange={(event) => control.setCopyOther(event.target.checked)}
          />
          <span>
            {t("import.other.copy")}
            <small>{t("import.other.copyHint", { count: survey?.other ?? 0 })}</small>
          </span>
        </label>

        {setup.error !== null && (
          <p className="ts-dialog__error" role="alert">
            {setup.error}
          </p>
        )}
      </div>

      <div className="ts-dialog__foot">
        <span className="ts-spacer" />
        <button type="button" className="ts-btn" onClick={control.closeDialog}>
          {t("common.cancel")}
        </button>
        <button
          type="button"
          className="ts-btn ts-btn--accent"
          disabled={!canStart}
          onClick={control.start}
        >
          {t("import.start", { count: survey?.notes ?? 0 })}
        </button>
      </div>
    </div>
  );
}
