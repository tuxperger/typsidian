# Contract: ядро — встраивание

```text
embeds::parse_target(raw) -> EmbedTarget
embeds::calls(text) -> Vec<EmbedCall { raw, string: Range, call: Range }>
embeds::plan(backend, note_path, text) -> EmbedPlan            // для сборки
embeds::extract(text, part) -> Option<String>                 // раздел/элемент
embeds::embed_library() / ensure_embed_library(backend) / embed_import_edits(text)
completion::completions — target Embed: группы Notes | Sections | Labels
graph::link_sites — сайты встраиваний с `call`
```

Клиент: `client.embeds.insert(path, text, offset)` → `{ edits: TextEdit[], library: LibraryStatus }`
— правки (`#import` при отсутствии + `#embed("")` у каретки) и запись файла библиотеки.
