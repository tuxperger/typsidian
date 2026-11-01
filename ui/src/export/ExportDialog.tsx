"use client";

import { useState } from "react";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

// Окно экспорта открытой заметки — артборд 10 нового дизайна (фича 044,
// `specs/044-ui-redesign-screens/design/Export.dc.html`).
//
// Своего экспорта у окна нет: PDF — прежний экспорт заметки (009), HTML —
// прежний пакет из одной заметки (034), «Слайды» ведут в прежний диалог
// презентации (035). Окно только даёт выбрать формат и видеть, что и куда
// уйдёт; пакетный Центр экспорта остаётся командой палитры.

export type ExportDialogFormat = "pdf" | "html" | "slides";

export interface ExportDialogProps {
  /** Путь открытой заметки от корня vault. */
  notePath: string;
  /** Страниц в последней сборке; `null` — сборки ещё не было. */
  pages: number | null;
  /** Папка экспорта vault — её значение решает ядро (`export_folder`). */
  folder: string;
  onExportPdf: () => void;
  onExportHtml: () => void;
  onOpenSlides: () => void;
  /** «Изменить…» — папка экспорта в настройках vault. */
  onChangeFolder: () => void;
  onClose: () => void;
}

const FORMATS = ["pdf", "html", "slides"] as const;

export function ExportDialog({
  notePath,
  pages,
  folder,
  onExportPdf,
  onExportHtml,
  onOpenSlides,
  onChangeFolder,
  onClose,
}: ExportDialogProps): React.JSX.Element {
  const { t } = useI18n();
  const [format, setFormat] = useState<ExportDialogFormat>("pdf");
  useDismissOnEscape(onClose);

  const name = notePath.slice(notePath.lastIndexOf("/") + 1);
  const run = (): void => {
    // Окно закрывается до действия: ход экспорта показывает шапка и
    // сообщение о результате, а диалог слайдов встаёт на его место.
    onClose();
    if (format === "pdf") onExportPdf();
    else if (format === "html") onExportHtml();
    else onOpenSlides();
  };

  return (
    <div
      className="ts-modal-scrim"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        className="ts-modal ts-export-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="ts-export-dialog-title"
      >
        <div className="ts-modal__head">
          <h2 id="ts-export-dialog-title" className="ts-modal__title">
            {t("export.dialog.title")}
          </h2>
          <button
            type="button"
            className="ts-modal__close"
            aria-label={t("export.dialog.close")}
            title={t("export.dialog.close")}
            onClick={onClose}
          >
            <Icon name="x" size={12} />
          </button>
        </div>

        <div className="ts-modal__body">
          <div className="ts-segmented" role="group" aria-label={t("export.dialog.format")}>
            {FORMATS.map((id) => (
              <button
                key={id}
                type="button"
                className="ts-segmented__item"
                aria-pressed={format === id}
                onClick={() => setFormat(id)}
              >
                {t(id === "pdf" ? "export.dialog.pdf" : id === "html" ? "export.dialog.html" : "export.dialog.slides")}
              </button>
            ))}
          </div>

          <div className="ts-modal__field">
            <label htmlFor="ts-export-what">{t("export.dialog.what")}</label>
            <input
              id="ts-export-what"
              className="ts-modal__input"
              type="text"
              readOnly
              value={t("export.dialog.whatValue", { name, pages })}
            />
          </div>

          {format === "slides" ? (
            <p className="ts-modal__note">{t("export.dialog.slidesNote")}</p>
          ) : (
            <div className="ts-modal__field">
              <label htmlFor="ts-export-where">{t("export.dialog.where")}</label>
              <div className="ts-modal__row">
                <input
                  id="ts-export-where"
                  className="ts-modal__input"
                  type="text"
                  readOnly
                  value={t("export.dialog.whereValue", { folder })}
                />
                <button type="button" className="ts-btn" onClick={onChangeFolder}>
                  {t("export.dialog.change")}
                </button>
              </div>
            </div>
          )}
        </div>

        <div className="ts-modal__foot">
          <button type="button" className="ts-btn ts-btn--quiet" onClick={onClose}>
            {t("export.dialog.cancel")}
          </button>
          {/* eslint-disable-next-line jsx-a11y/no-autofocus -- окно открыто
              ради одного действия; Enter его и выполняет */}
          <button type="button" className="ts-btn ts-btn--accent" autoFocus onClick={run}>
            {t(
              format === "pdf"
                ? "export.dialog.savePdf"
                : format === "html"
                  ? "export.dialog.saveHtml"
                  : "export.dialog.nextSlides",
            )}
          </button>
        </div>
      </section>
    </div>
  );
}
