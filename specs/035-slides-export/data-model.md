# Data Model: Экспорт презентации

```text
SlidesFormat   = Pdf | PdfWithNotes | PdfPc | Png | Svg
StepChoice     = LastStep | AllSteps
SlidesExportRequest { format, steps, png_scale: u32 (1..=4) }
SlidesExportReport  { note_path, outcome: Written { files: [String], pages: u32 }
                      | CompileFailed | NoSlides | WriteFailed { reason } | NoteMissing | NoteUnreadable,
                      diagnostics }
```
