//! wasm-биндинги (фича 050). Тонкие обёртки. Сетевые функции не принимают
//! `WebBackend`: на время запроса он не должен быть занят (`wasm-call-queue.ts`).

use wasm_bindgen::prelude::*;

use super::access::check_access;
use super::cancel::{cancel, cancellable};
use super::endpoint::base_url;
use super::client::{
    approve_endpoint, begin_probe, finish_probe, prepare_models, provider_key, require_approved,
    send_models, send_probe, PreparedProbe, ProbeResult,
};
use super::journal::{journal_clear, journal_entries, JournalEntry};
use super::protocol::ProbeReply;
use super::settings::{load_settings, save_settings, AiProvider, AiSettings};
use super::transport::DefaultTransport;
use super::AiFailure;
use crate::sync::secrets::web::IdbSecretStore;
use crate::sync::secrets::SecretStore;
use crate::vault::web::WebBackend;

#[wasm_bindgen(js_name = aiSettings)]
pub async fn ai_settings_js(backend: &WebBackend) -> AiSettings {
    load_settings(backend).await
}

#[wasm_bindgen(js_name = aiSetSettings)]
pub async fn ai_set_settings_js(
    backend: &mut WebBackend,
    settings: AiSettings,
) -> Result<(), AiFailure> {
    save_settings(backend, &settings).await
}

#[wasm_bindgen(js_name = aiKeyState)]
pub async fn ai_key_state_js(vault_id: String, provider: AiProvider) -> Result<bool, AiFailure> {
    IdbSecretStore
        .get(&vault_id, provider.secret_kind())
        .await
        .map(|key| key.is_some())
        .map_err(|_| AiFailure::SecretsUnavailable)
}

#[wasm_bindgen(js_name = aiKeySet)]
pub async fn ai_key_set_js(
    vault_id: String,
    provider: AiProvider,
    key: String,
) -> Result<(), AiFailure> {
    IdbSecretStore
        .set(&vault_id, provider.secret_kind(), &key)
        .await
        .map_err(|_| AiFailure::SecretsUnavailable)
}

#[wasm_bindgen(js_name = aiKeyDelete)]
pub async fn ai_key_delete_js(vault_id: String, provider: AiProvider) -> Result<(), AiFailure> {
    IdbSecretStore
        .delete(&vault_id, provider.secret_kind())
        .await
        .map_err(|_| AiFailure::SecretsUnavailable)
}

/// Свой адрес должен быть одобрен на этом устройстве (ревью безопасности 050, M1):
/// проверка до журнала, как и отсутствие ключа.
#[wasm_bindgen(js_name = aiBeginProbe)]
pub async fn ai_begin_probe_js(
    backend: &mut WebBackend,
    vault_id: String,
    id: String,
    now: f64,
) -> Result<PreparedProbe, AiFailure> {
    let settings = load_settings(backend).await;
    if settings.enabled {
        base_url(&settings, true)?;
        require_approved(&IdbSecretStore, &vault_id, &settings).await?;
    }
    begin_probe(backend, &id, now as i64, true).await
}

/// Сеть. Отказ возвращается значением, чтобы итог попал в журнал следующим шагом.
#[wasm_bindgen(js_name = aiSendProbe)]
pub async fn ai_send_probe_js(
    vault_id: String,
    prepared: PreparedProbe,
) -> Result<ProbeReply, AiFailure> {
    let key = provider_key(&IdbSecretStore, &vault_id, prepared.provider).await?;
    cancellable(
        &vault_id,
        &prepared.id,
        send_probe(&DefaultTransport, &prepared, key.as_deref()),
    )
    .await
}

#[wasm_bindgen(js_name = aiFinishProbe)]
pub async fn ai_finish_probe_js(
    backend: &mut WebBackend,
    prepared: PreparedProbe,
    now: f64,
    reply: Option<ProbeReply>,
    failure: Option<AiFailure>,
) -> Result<ProbeResult, AiFailure> {
    let outcome = reply.ok_or(failure.unwrap_or(AiFailure::BadResponse));
    finish_probe(backend, &prepared, now as i64, outcome).await
}

/// Настройки передаёт клиент: прочитаны предыдущим вызовом, vault здесь не занят.
#[wasm_bindgen(js_name = aiModels)]
pub async fn ai_models_js(
    vault_id: String,
    request_id: String,
    settings: AiSettings,
) -> Result<Vec<String>, AiFailure> {
    let request = prepare_models(&settings, true)?;
    require_approved(&IdbSecretStore, &vault_id, &settings).await?;
    let key = provider_key(&IdbSecretStore, &vault_id, settings.provider).await?;
    cancellable(
        &vault_id,
        &request_id,
        send_models(
            &DefaultTransport,
            settings.provider,
            request,
            key.as_deref(),
        ),
    )
    .await
}

#[wasm_bindgen(js_name = aiApproveEndpoint)]
pub async fn ai_approve_endpoint_js(
    backend: &WebBackend,
    vault_id: String,
    insecure_ok: bool,
) -> Result<(), AiFailure> {
    let settings = load_settings(backend).await;
    approve_endpoint(&IdbSecretStore, &vault_id, &settings, insecure_ok).await
}

#[wasm_bindgen(js_name = aiCancel)]
pub fn ai_cancel_js(vault_id: String, request_id: String) {
    cancel(&vault_id, &request_id);
}

#[wasm_bindgen(js_name = aiJournal)]
pub async fn ai_journal_js(backend: &WebBackend) -> Vec<JournalEntry> {
    journal_entries(backend).await
}

#[wasm_bindgen(js_name = aiJournalClear)]
pub async fn ai_journal_clear_js(backend: &mut WebBackend) -> Result<u32, AiFailure> {
    journal_clear(backend).await
}

#[wasm_bindgen(js_name = aiCheckAccess)]
pub fn ai_check_access_js(path: String, settings: AiSettings) -> bool {
    check_access(&path, &settings)
}
