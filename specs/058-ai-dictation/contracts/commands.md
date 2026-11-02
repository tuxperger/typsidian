# Contracts: диктовка

| Команда | Вход | Выход |
| --- | --- | --- |
| `ai_speech_transcribe` | `vaultId`, `requestId`, `audio` (байты; на сервере base64), `mime`, `seconds`, `now`, `allowCustom` | `Transcription` |
| `ai_speech_approve_endpoint` | `vaultId`, `insecureOk` | — |
| `ai_speech_key_{state,set,delete}` | `vaultId` | как у 050 |

`ai_speech_transcribe` — в `AI_NETWORK_COMMANDS`. wasm: `speechBegin/Send/Finish`
(как пробный запрос 050), `speechApproveEndpoint`, `speechKey*`.
