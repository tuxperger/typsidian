# Tasks: Диктовка ответов в уроке

**Input**: `specs/058-ai-dictation/` (spec, plan, research, data-model, contracts)

**Tests**: в `packages/core/tests/common/speech_cases.rs`, на обоих путях.

**Зависит от**: 051 (поля ответа); 053 — по мере готовности.

## Phase 1: Основа

- [ ] T001 `SpeechSettings` в `packages/core/src/ai/settings.rs`; секреты `AiSpeechCustomKey`, `AiSpeechCustomOrigin` в `packages/core/src/sync/secrets.rs` и `forget_vault`; `body_bytes` у `HttpRequest` на обоих транспортах в `packages/core/src/ai/transport.rs`

## Phase 2: US1 — надиктовать рассуждение (P1)

**Goal**: запись → распознавание → текст в поле.
**Independent Test**: поддельный транспорт: multipart, журнал без звука, текст в ответе.

- [ ] T002 [US1] Распознавание: одобрение своего адреса, ключ, multipart, журнал с описанием звука и `reply_text` (research §1, §3, §4) в `packages/core/src/ai/{speech,journal}.rs`
- [ ] T003 [US1] Тесты: multipart; журнал без байтов; `EndpointNotApproved`; `None` — `Disabled`; отказы 050
- [ ] T004 [US1] Команды `ai_speech_*` (host, shell, сервер: белый список, предел, флаг своего адреса, base64; wasm); клиент; локали
- [ ] T005 [US1] Запись `MediaRecorder` с пределом, предупреждением и отменой, кнопка микрофона в полях ответа и рассуждения урока и практики по макету A–D в `ui/src/lessons/{DictationButton,useRecorder}.tsx`; вставка на место курсора
- [ ] T006 [US1] Разрешения микрофона: WebKitGTK на рабочем столе, `RECORD_AUDIO` и выдача разрешения WebView на Android (`tauri-shell-agent`) в `apps/desktop/src-tauri/`, `apps/mobile/src-tauri/`

## Phase 3: US2 — язык распознавания (P2)

- [ ] T007 [US2] Блок «Распознавание речи» по макету E (подключение, ключ, модель, язык) в `ui/src/settings/AiSpeechSection.tsx`; язык передаётся в запрос; тест

## Phase 4: Завершение

- [ ] T008 Проверка на трёх оболочках руками (микрофон, текст в поле, нет файлов звука — SC-002)
- [ ] T009 Гейты `gates.sh all`
- [ ] T010 Проверка `security-review-agent`, правка находок
- [ ] T011 `/speckit-converge`, README, `**Status**: Implemented`
