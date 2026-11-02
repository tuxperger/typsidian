//! wasm-биндинги урока (фича 051, contracts/commands.md). Тонкие обёртки, как в
//! `ai/web.rs`. Сетевая `lessonSend` не принимает `WebBackend`: на время запроса он не
//! должен быть занят (`wasm-call-queue.ts`). Цикл «начать — отправить — завершить»
//! крутит клиент, а решение «продолжать или ждать» возвращает `lessonFinish`
//! (research.md §13).

use wasm_bindgen::prelude::*;

use super::file::{Language, LessonState, LessonSummary, LessonView};
use super::partial::{checkpoint, progress, LessonProgress};
use super::stream::TurnReply;
use super::turn::{begin_turn, finish_turn, send_turn, PreparedTurn, TurnDone, UserAnswer};
use super::{create_lesson, list_lessons, open_lesson, set_language, set_state};
use crate::ai::client::{provider_key, require_approved, Connection};
use crate::ai::settings::load_settings;
use crate::ai::transport::DefaultTransport;
use crate::ai::AiFailure;
use crate::sync::secrets::web::IdbSecretStore;
use crate::vault::web::WebBackend;

#[wasm_bindgen(js_name = lessonList)]
pub async fn lesson_list_js(backend: &WebBackend) -> Vec<LessonSummary> {
    list_lessons(backend).await
}

#[wasm_bindgen(js_name = lessonCreate)]
pub async fn lesson_create_js(
    backend: &mut WebBackend,
    goal: String,
    notes: Vec<String>,
    language: Language,
    now: f64,
) -> Result<String, AiFailure> {
    create_lesson(backend, &goal, &notes, language, now as i64).await
}

#[wasm_bindgen(js_name = lessonOpen)]
pub async fn lesson_open_js(
    backend: &mut WebBackend,
    path: String,
) -> Result<LessonView, AiFailure> {
    open_lesson(backend, &path).await
}

/// Свой адрес должен быть одобрен, ключ — задан: проверки до записи ответа и журнала,
/// как у `aiBeginProbe`. `answer: null` — повтор или автопродолжение.
#[wasm_bindgen(js_name = lessonBegin)]
#[allow(clippy::too_many_arguments)]
pub async fn lesson_begin_js(
    backend: &mut WebBackend,
    vault_id: String,
    path: String,
    request_id: String,
    answer: Option<UserAnswer>,
    expected: u32,
    now: f64,
) -> Result<PreparedTurn, AiFailure> {
    let settings = load_settings(backend).await;
    if !settings.enabled {
        return Err(AiFailure::Disabled);
    }
    let verified = Connection::verify(&settings, true)?;
    require_approved(&IdbSecretStore, &vault_id, &settings).await?;
    provider_key(&IdbSecretStore, &vault_id, settings.provider).await?;
    begin_turn(
        backend,
        &path,
        &request_id,
        answer,
        expected,
        now as i64,
        true,
        &verified,
    )
    .await
}

/// Сеть. Отказ возвращается значением, чтобы итог попал в файл и журнал следующим шагом.
#[wasm_bindgen(js_name = lessonSend)]
pub async fn lesson_send_js(
    vault_id: String,
    prepared: PreparedTurn,
) -> Result<TurnReply, AiFailure> {
    let key = provider_key(&IdbSecretStore, &vault_id, prepared.provider).await?;
    let verified = Connection::of_request(prepared.provider, &prepared.request.url);
    send_turn(
        &DefaultTransport,
        &vault_id,
        &prepared,
        key.as_deref(),
        &verified,
    )
    .await
}

#[wasm_bindgen(js_name = lessonFinish)]
pub async fn lesson_finish_js(
    backend: &mut WebBackend,
    vault_id: String,
    prepared: PreparedTurn,
    now: f64,
    reply: Option<TurnReply>,
    failure: Option<AiFailure>,
) -> Result<TurnDone, AiFailure> {
    let outcome = reply.ok_or(failure.unwrap_or(AiFailure::BadResponse));
    finish_turn(backend, &vault_id, &prepared, now as i64, outcome).await
}

/// Опрос идущего хода раз в 150 мс.
#[wasm_bindgen(js_name = lessonProgress)]
pub fn lesson_progress_js(vault_id: String, request_id: String) -> Option<LessonProgress> {
    progress(&vault_id, &request_id)
}

/// Раз в секунду, пока идёт ход: переписывает промежуточный файл (research.md §5).
#[wasm_bindgen(js_name = lessonCheckpoint)]
pub async fn lesson_checkpoint_js(
    backend: &mut WebBackend,
    vault_id: String,
    path: String,
    request_id: String,
) -> Result<(), AiFailure> {
    checkpoint(backend, &vault_id, &path, &request_id).await
}

#[wasm_bindgen(js_name = lessonSetState)]
pub async fn lesson_set_state_js(
    backend: &mut WebBackend,
    path: String,
    state: LessonState,
    now: f64,
) -> Result<LessonView, AiFailure> {
    set_state(backend, &path, state, now as i64).await
}

#[wasm_bindgen(js_name = lessonSetLanguage)]
pub async fn lesson_set_language_js(
    backend: &mut WebBackend,
    path: String,
    language: Language,
    now: f64,
) -> Result<LessonView, AiFailure> {
    set_language(backend, &path, language, now as i64).await
}
