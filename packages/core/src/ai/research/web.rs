//! wasm-биндинги исследования (фича 056, contracts/commands.md). Тонкие обёртки, как в
//! `knowledge/web.rs`. Сетевая `researchSend` не принимает `WebBackend`: на время запроса он
//! не должен быть занят (`wasm-call-queue.ts`). Цикл «начать — отправить — завершить»
//! крутит клиент: `researchBegin` возвращает `Queued` (повторить позже), `Ready` (отправить)
//! или `Finished`; `researchFinish` — `next: Continue` с новым `prepared`, если модель
//! попросила продолжить. Запуск исследования, запрошенного учителем, — тоже за клиентом, по
//! признаку `launchDue` в `LessonView.research`.

use wasm_bindgen::prelude::*;

use super::{
    begin_research, finish_research, research_estimate, send_research, PreparedResearch,
    ResearchBegin, ResearchEstimate, ResearchFinish, ResearchKind, ResearchReply,
};
use crate::ai::client::{provider_key, require_approved, Connection};
use crate::ai::jobs::{all_jobs, AnyJob};
use crate::ai::settings::load_settings;
use crate::ai::transport::DefaultTransport;
use crate::ai::AiFailure;
use crate::sync::secrets::web::IdbSecretStore;
use crate::vault::web::WebBackend;

/// Оценка чтения PDF: только чтение vault, без сети.
#[wasm_bindgen(js_name = researchEstimate)]
pub async fn research_estimate_js(
    backend: &WebBackend,
    pdf: String,
) -> Result<ResearchEstimate, AiFailure> {
    research_estimate(backend, &pdf).await
}

/// Свой адрес должен быть одобрен, ключ — задан: проверки до записи журнала, как у
/// `knowledgeBegin`.
#[wasm_bindgen(js_name = researchBegin)]
#[allow(clippy::too_many_arguments)]
pub async fn research_begin_js(
    backend: &mut WebBackend,
    vault_id: String,
    path: String,
    request_id: String,
    kind: ResearchKind,
    question: String,
    pdf: Option<String>,
    now: f64,
) -> Result<ResearchBegin, AiFailure> {
    let settings = load_settings(backend).await;
    if !settings.enabled {
        return Err(AiFailure::Disabled);
    }
    let verified = Connection::verify(&settings, true)?;
    require_approved(&IdbSecretStore, &vault_id, &settings).await?;
    provider_key(&IdbSecretStore, &vault_id, settings.provider).await?;
    begin_research(
        backend,
        &vault_id,
        &path,
        &request_id,
        kind,
        &question,
        pdf.as_deref(),
        now as i64,
        true,
        &verified,
    )
    .await
}

/// Сеть. Отказ возвращается значением, чтобы итог попал в файл урока и журнал следующим шагом.
#[wasm_bindgen(js_name = researchSend)]
pub async fn research_send_js(
    vault_id: String,
    prepared: PreparedResearch,
) -> Result<ResearchReply, AiFailure> {
    let key = provider_key(&IdbSecretStore, &vault_id, prepared.provider).await?;
    let verified = Connection::of_request(prepared.provider, &prepared.request.url);
    send_research(
        &DefaultTransport,
        &vault_id,
        &prepared,
        key.as_deref(),
        &verified,
    )
    .await
}

#[wasm_bindgen(js_name = researchFinish)]
pub async fn research_finish_js(
    backend: &mut WebBackend,
    vault_id: String,
    prepared: PreparedResearch,
    now: f64,
    reply: Option<ResearchReply>,
    failure: Option<AiFailure>,
) -> Result<ResearchFinish, AiFailure> {
    let outcome = reply.ok_or(failure.unwrap_or(AiFailure::BadResponse));
    finish_research(backend, &vault_id, &prepared, now as i64, outcome).await
}

/// Все фоновые задания vault (пополнение графа знаний и исследования): идущие первыми. Опрос.
#[wasm_bindgen(js_name = aiJobs)]
pub fn ai_jobs_js(vault_id: String) -> Vec<AnyJob> {
    all_jobs(&vault_id)
}
