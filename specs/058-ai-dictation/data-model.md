# Data Model: Диктовка

- **SpeechSettings** (в `AiSettings.speech`): `provider` (`None`/`OpenAi`/`Custom`),
  `endpoint?`, `model` (`whisper-1`), `language` (`Auto` или код языка).
- Секреты: `AiOpenAiKey` (050), `AiSpeechCustomKey`, `AiSpeechCustomOrigin`.
- **Transcription**: `text`, `seconds`.
- `HttpRequest.body_bytes?` — двоичное тело (050 расширяется).
- Журнал 050: `Sent.request` — описание звука; `Done.reply_text?`.
