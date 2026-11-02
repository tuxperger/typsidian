# Data Model: Инструкции агентов

- **Agent**: `Teacher` / `Probe` / `Practice` / `Extract` / `Compact` / `Research`.
- **BuiltinPrompt** (ядро): агент, язык, текст, версия.
- **PromptOverride** (`.typsidian/ai-prompts.json`): агент, язык, текст (непустой),
  `base_version`.
- **PromptView**: агент, язык, действующий текст, `edited`, `base_updated`, исходный текст
  (для сравнения), оценка единиц.
