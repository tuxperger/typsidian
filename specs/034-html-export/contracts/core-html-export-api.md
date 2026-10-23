# Contract: ядро — экспорт в HTML

```text
export_note_html(backend, packages, note_path, source_text, fonts, overrides, site, labels) -> ExportReport
export_site_index(backend, overrides, site, labels) -> Result<String, ExportWriteProblem>   // путь index.html
export_site_archive(backend, overrides, site) -> Vec<u8>                                    // zip
html_path(note, folder) -> PathBuf
batch::start(paths, stop_on_error, overrides, format)
```
