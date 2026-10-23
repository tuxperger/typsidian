"use client";

import { useState } from "react";
import type {
  SlidesExportReport,
  SlidesFormat,
  StepChoice,
  VaultOverrides,
} from "../wasm-core/typsidian_core.js";
import type { PreviewClient } from "../preview-client.js";
import { Icon } from "../icons/Icons.js";
import { useI18n, type MessageKey } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

// Окно «Экспорт презентации» (фича 035, макет P1). Слайды, шаги, PDF, pdfpc и
// картинки считает ядро (`slides::export`); здесь — выбор и итог.

export interface SlidesExportDialogProps {
  client: PreviewClient;
  notePath: string;
  /** Текст открытой заметки: экспортируется то, что в редакторе. */
  text: () => string | null;
  overrides: VaultOverrides;
  onClose: () => void;
}

const FORMATS: { id: SlidesFormat; title: MessageKey; hint: MessageKey }[] = [
  { id: "Pdf", title: "slides.export.pdf", hint: "slides.export.pdfHint" },
  { id: "PdfWithNotes", title: "slides.export.notes", hint: "slides.export.notesHint" },
  { id: "PdfPc", title: "slides.export.pdfpc", hint: "slides.export.pdfpcHint" },
  { id: "Png", title: "slides.export.png", hint: "slides.export.pngHint" },
  { id: "Svg", title: "slides.export.svg", hint: "slides.export.svgHint" },
];

export function SlidesExportDialog({
  client,
  notePath,
  text,
  overrides,
  onClose,
}: SlidesExportDialogProps): React.JSX.Element {
  const { t } = useI18n();
  useDismissOnEscape(onClose);
  const [format, setFormat] = useState<SlidesFormat>("Pdf");
  const [steps, setSteps] = useState<StepChoice>("LastStep");
  const [scale, setScale] = useState(2);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<SlidesExportReport | null>(null);

  const run = async (): Promise<void> => {
    setBusy(true);
    setResult(null);
    try {
      setResult(
        await client.exportSlides(notePath, text(), overrides, {
          format,
          steps,
          png_scale: scale,
        }),
      );
    } catch (cause) {
      setResult({
        note_path: notePath,
        outcome: { kind: "CompileFailed" },
        diagnostics: [{ severity: "Error", message: String(cause), location: undefined }],
      });
    } finally {
      setBusy(false);
    }
  };

  const outcome = result?.outcome;
  return (
    <div className="ts-overlay" role="presentation">
      <section className="ts-search ts-slides-export" aria-label={t("slides.export.title")}>
        <div className="ts-search__query">
          <span className="ts-history__title">{t("slides.export.title")}</span>
          <span className="ts-search__hit-meta">{notePath}</span>
          <span className="ts-spacer" />
          <button
            type="button"
            className="ts-btn ts-btn--ghost"
            aria-label={t("slides.export.close")}
            onClick={onClose}
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        <div className="ts-slides-export__formats" role="radiogroup" aria-label={t("slides.export.format")}>
          {FORMATS.map((option) => (
            <label
              key={option.id}
              className={
                format === option.id
                  ? "ts-slides-export__format ts-slides-export__format--on"
                  : "ts-slides-export__format"
              }
            >
              <input
                id={`ts-slides-format-${option.id}`}
                type="radio"
                name="ts-slides-format"
                checked={format === option.id}
                onChange={() => setFormat(option.id)}
              />
              <span>
                <span className="ts-slides-export__name">{t(option.title)}</span>
                <span className="ts-slides-export__hint">{t(option.hint)}</span>
              </span>
            </label>
          ))}
        </div>

        <div className="ts-slides-export__line">
          <span className="ts-eyebrow">{t("slides.export.steps")}</span>
          {(["LastStep", "AllSteps"] as const).map((id) => (
            <button
              key={id}
              type="button"
              className="ts-chip"
              aria-pressed={(format === "PdfPc" ? "AllSteps" : steps) === id}
              disabled={format === "PdfPc"}
              onClick={() => setSteps(id)}
            >
              {id === "LastStep" ? t("slides.export.lastStep") : t("slides.export.allSteps")}
            </button>
          ))}
        </div>
        {format === "Png" && (
          <div className="ts-slides-export__line">
            <span className="ts-eyebrow">{t("slides.export.scale")}</span>
            {[1, 2, 3, 4].map((value) => (
              <button
                key={value}
                type="button"
                className="ts-chip"
                aria-pressed={scale === value}
                onClick={() => setScale(value)}
              >
                {`${value}×`}
              </button>
            ))}
          </div>
        )}

        {outcome !== undefined && (
          <div
            className={
              outcome.kind === "Written"
                ? "ts-replace__panel"
                : "ts-replace__panel ts-replace__panel--warn"
            }
            role="status"
          >
            {outcome.kind === "Written" ? (
              <>
                <span className="ts-replace__title">
                  {t("slides.export.done", { count: outcome.pages })}
                </span>
                {outcome.files.slice(0, 6).map((file) => (
                  <span key={file} className="ts-replace__meta">
                    {file}
                  </span>
                ))}
                {outcome.files.length > 6 && (
                  <span className="ts-replace__meta">
                    {t("slides.export.more", { count: outcome.files.length - 6 })}
                  </span>
                )}
              </>
            ) : (
              <>
                <span className="ts-replace__title">
                  {outcome.kind === "NoSlides"
                    ? t("slides.export.noSlides")
                    : outcome.kind === "WriteFailed"
                      ? t("slides.export.writeFailed")
                      : t("slides.export.failed")}
                </span>
                {(result?.diagnostics ?? []).slice(0, 3).map((diagnostic, index) => (
                  <span key={index} className="ts-replace__meta">
                    {diagnostic.message}
                  </span>
                ))}
              </>
            )}
          </div>
        )}

        <div className="ts-search__hints ts-replace__foot">
          <span>{t("slides.export.folder")}</span>
          <span className="ts-spacer" />
          <button type="button" className="ts-btn ts-btn--quiet" onClick={onClose}>
            {t("slides.export.close")}
          </button>
          <button type="button" className="ts-btn ts-btn--accent" disabled={busy} onClick={() => void run()}>
            {busy ? t("slides.export.running") : t("slides.export.start")}
          </button>
        </div>
      </section>
    </div>
  );
}
