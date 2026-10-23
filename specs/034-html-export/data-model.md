# Data Model: Экспорт в HTML

- `ExportFormat` = `Pdf` | `Html`; `BatchState.format` (по умолчанию `Pdf`).
- `ExportReport.pdf_path` — путь результата (у HTML — `.html`); `as_images: Vec<String>` —
  элементы, выведенные рисунком (FR-003).
- `SiteLabels { contents, outside }` — подписи навигации из переводов.
- Путь страницы: `<папка экспорта>/<путь заметки без .typ>.html`; оглавление —
  `<папка экспорта>/index.html`.
