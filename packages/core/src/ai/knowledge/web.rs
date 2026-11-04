//! wasm-биндинги графа знаний (фича 054, contracts/commands.md). Тонкие обёртки, как в
//! `lesson/web.rs`. Сетевая `knowledgeSend` не принимает `WebBackend`: на время запроса
//! он не должен быть занят (`wasm-call-queue.ts`). Цикл «начать — отправить — завершить»
//! по пачкам крутит клиент, а решение «следующая пачка или конец» возвращает
//! `knowledgeFinish` (`next`). Запуск после хода учителя — тоже за клиентом, по признаку
//! `LessonView.knowledgeAutoDue`.

use wasm_bindgen::prelude::*;

use super::extract::{begin_extract, finish_extract, send_extract, ExtractDone, PreparedExtract};
use super::import::{begin_import, finish_import, import_estimate, ImportEstimate};
use super::jobs::{jobs, KnowledgeJob};
use super::view::{Chain, KnowledgeGraph, Proposition};
use super::{
    chains_view, delete_lesson, graph_view, proposition_view, remove_proposition, remove_source,
};
use crate::ai::client::{provider_key, require_approved, Connection};
use crate::ai::lesson::stream::TurnReply;
use crate::ai::settings::load_settings;
use crate::ai::transport::DefaultTransport;
use crate::ai::AiFailure;
use crate::sync::secrets::web::IdbSecretStore;
use crate::vault::web::WebBackend;

#[wasm_bindgen(js_name = knowledgeGraph)]
pub async fn knowledge_graph_js(backend: &WebBackend, query: Option<String>) -> KnowledgeGraph {
    graph_view(backend, query.as_deref().unwrap_or_default()).await
}

#[wasm_bindgen(js_name = knowledgeChains)]
pub async fn knowledge_chains_js(backend: &WebBackend, query: Option<String>) -> Vec<Chain> {
    chains_view(backend, query.as_deref().unwrap_or_default()).await
}

#[wasm_bindgen(js_name = knowledgeProposition)]
pub async fn knowledge_proposition_js(backend: &WebBackend, id: String) -> Option<Proposition> {
    proposition_view(backend, &id).await
}

/// Свой адрес должен быть одобрен, ключ — задан: проверки до записи журнала, как у
/// `lessonBegin`. `undefined` — разбирать больше нечего или задание отменено.
#[wasm_bindgen(js_name = knowledgeBegin)]
pub async fn knowledge_begin_js(
    backend: &mut WebBackend,
    vault_id: String,
    source: String,
    request_id: String,
    now: f64,
) -> Result<Option<PreparedExtract>, AiFailure> {
    let settings = load_settings(backend).await;
    if !settings.enabled {
        return Err(AiFailure::Disabled);
    }
    let verified = Connection::verify(&settings, true)?;
    require_approved(&IdbSecretStore, &vault_id, &settings).await?;
    provider_key(&IdbSecretStore, &vault_id, settings.provider).await?;
    begin_extract(
        backend,
        &vault_id,
        &source,
        &request_id,
        now as i64,
        true,
        &verified,
    )
    .await
}

/// Сеть. Отказ возвращается значением, чтобы итог попал в задание и журнал следующим шагом.
#[wasm_bindgen(js_name = knowledgeSend)]
pub async fn knowledge_send_js(
    vault_id: String,
    prepared: PreparedExtract,
) -> Result<TurnReply, AiFailure> {
    let key = provider_key(&IdbSecretStore, &vault_id, prepared.provider).await?;
    let verified = Connection::of_request(prepared.provider, &prepared.request.url);
    send_extract(
        &DefaultTransport,
        &vault_id,
        &prepared,
        key.as_deref(),
        &verified,
    )
    .await
}

#[wasm_bindgen(js_name = knowledgeFinish)]
pub async fn knowledge_finish_js(
    backend: &mut WebBackend,
    vault_id: String,
    prepared: PreparedExtract,
    now: f64,
    reply: Option<TurnReply>,
    failure: Option<AiFailure>,
) -> Result<ExtractDone, AiFailure> {
    let outcome = reply.ok_or(failure.unwrap_or(AiFailure::BadResponse));
    finish_extract(backend, &vault_id, &prepared, now as i64, outcome).await
}

/// Задания пополнения графа: идущие первыми. Опрос.
#[wasm_bindgen(js_name = knowledgeJobs)]
pub fn knowledge_jobs_js(vault_id: String) -> Vec<KnowledgeJob> {
    jobs(&vault_id)
}

/// Утверждение (`id`) либо вклад источника (`source`); ровно одно из двух.
#[wasm_bindgen(js_name = knowledgeRemove)]
pub async fn knowledge_remove_js(
    backend: &mut WebBackend,
    id: Option<String>,
    source: Option<String>,
    keep: bool,
    now: f64,
) -> Result<(), AiFailure> {
    match (id, source) {
        (Some(id), None) => remove_proposition(backend, &id, now as i64).await,
        (None, Some(source)) => remove_source(backend, &source, keep, now as i64).await,
        _ => Err(AiFailure::LessonUnreadable),
    }
}

/// Удаляет файл урока и решает судьбу вклада в граф знаний.
#[wasm_bindgen(js_name = lessonDelete)]
pub async fn lesson_delete_js(
    backend: &mut WebBackend,
    path: String,
    keep_knowledge: bool,
    now: f64,
) -> Result<(), AiFailure> {
    delete_lesson(backend, None, &path, keep_knowledge, now as i64).await
}

/// Оценка импорта (фича 055): только чтение vault, без сети.
#[wasm_bindgen(js_name = knowledgeImportEstimate)]
pub async fn knowledge_import_estimate_js(
    backend: &WebBackend,
    paths: Vec<String>,
) -> Result<ImportEstimate, AiFailure> {
    import_estimate(backend, &paths).await
}

/// Начало части импорта: те же проверки, что у `knowledgeBegin`. Первый вызов с
/// `request_id` пересчитывает оценку и заводит задание (`LessonChanged`, если она изменилась);
/// дальше цикл тот же: `knowledgeSend`, потом `knowledgeImportFinish`. `undefined` —
/// отправлять больше нечего или задание отменено.
#[wasm_bindgen(js_name = knowledgeImportBegin)]
pub async fn knowledge_import_begin_js(
    backend: &mut WebBackend,
    vault_id: String,
    request_id: String,
    estimate: ImportEstimate,
    now: f64,
) -> Result<Option<PreparedExtract>, AiFailure> {
    let settings = load_settings(backend).await;
    if !settings.enabled {
        return Err(AiFailure::Disabled);
    }
    let verified = Connection::verify(&settings, true)?;
    require_approved(&IdbSecretStore, &vault_id, &settings).await?;
    provider_key(&IdbSecretStore, &vault_id, settings.provider).await?;
    begin_import(
        backend,
        &vault_id,
        &request_id,
        &estimate,
        now as i64,
        true,
        &verified,
    )
    .await
}

#[wasm_bindgen(js_name = knowledgeImportFinish)]
pub async fn knowledge_import_finish_js(
    backend: &mut WebBackend,
    vault_id: String,
    prepared: PreparedExtract,
    now: f64,
    reply: Option<TurnReply>,
    failure: Option<AiFailure>,
) -> Result<ExtractDone, AiFailure> {
    let outcome = reply.ok_or(failure.unwrap_or(AiFailure::BadResponse));
    finish_import(backend, &vault_id, &prepared, now as i64, outcome).await
}
