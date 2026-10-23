# Contract: ядро — экспорт презентации

```text
slides::export::selected_pages(slides, steps) -> Vec<(page, slide_no, step_no)>
slides::export::pdfpc_json(slides) -> String
slides::export::export_slides(backend, packages, note, text, fonts, overrides, request) -> SlidesExportReport
```

Клиент: `previewClient.exportSlides(notePath, text, overrides, request)`.
