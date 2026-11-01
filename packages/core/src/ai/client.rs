//! Запросы к модели: журнал до отправки, сеть, итог (см. описание модуля).

use serde::{Deserialize, Serialize};

use super::endpoint::{base_url, classify_endpoint, EndpointClass};
use super::journal::{record_done, record_sent};
use super::price::estimate_usd;
use super::protocol::{
    authorize, check_status, models_request, parse_models, parse_probe, probe_request, ProbeReply,
};
use super::settings::{load_settings, AiProvider, AiSettings};
use super::transport::{AiTransport, HttpRequest};
use super::AiFailure;
use crate::sync::secrets::{SecretKind, SecretStore};
use crate::vault::backend::VaultBackend;

/// Пробный запрос, записанный в журнал. Ключа не содержит.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PreparedProbe {
    pub id: String,
    pub provider: AiProvider,
    pub model: String,
    pub request: HttpRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ProbeResult {
    pub reply: ProbeReply,
    pub cost_usd: Option<f64>,
}

/// Шаг 1. Выключенная функция отказывает до любой сети (FR-002), отказ записи
/// журнала останавливает запрос (FR-012).
pub async fn begin_probe<B: VaultBackend>(
    backend: &mut B,
    id: &str,
    now: i64,
    allow_custom: bool,
) -> Result<PreparedProbe, AiFailure> {
    let settings = load_settings(backend).await;
    if !settings.enabled {
        return Err(AiFailure::Disabled);
    }
    let base = base_url(&settings, allow_custom)?;
    let model = settings.model.clone().ok_or(AiFailure::ModelMissing)?;
    let request = probe_request(settings.provider, &base, &model);
    record_sent(
        backend,
        id,
        now,
        settings.provider,
        &model,
        &request.url,
        request.body.as_deref().unwrap_or_default(),
    )
    .await?;
    Ok(PreparedProbe {
        id: id.to_owned(),
        provider: settings.provider,
        model,
        request,
    })
}

/// Ключ подключения или `None`, если он не обязателен.
pub async fn provider_key<S: SecretStore>(
    secrets: &S,
    vault_id: &str,
    provider: AiProvider,
) -> Result<Option<String>, AiFailure> {
    let key = secrets
        .get(vault_id, provider.secret_kind())
        .await
        .map_err(|_| AiFailure::SecretsUnavailable)?;
    if key.is_none() && provider.requires_key() {
        return Err(AiFailure::KeyMissing);
    }
    Ok(key)
}

/// Адрес своего подключения в той форме, в какой его одобряют и сверяют.
fn custom_origin(settings: &AiSettings) -> Option<String> {
    settings
        .custom_endpoint
        .as_deref()
        .map(|endpoint| endpoint.trim().trim_end_matches('/').to_owned())
}

/// Одобряет на этом устройстве свой адрес из настроек vault (ревью безопасности
/// 050, M1). Зовётся только по явному действию пользователя здесь: ввод адреса,
/// подтверждение незашифрованного соединения, «доверять адресу». Незашифрованный
/// адрес вне устройства одобряется только вместе с подтверждением `insecure_ok`
/// (FR-010) — пришедшее с синхронизацией подтверждение не в счёт.
pub async fn approve_endpoint<S: SecretStore>(
    secrets: &S,
    vault_id: &str,
    settings: &AiSettings,
    insecure_ok: bool,
) -> Result<(), AiFailure> {
    if settings.provider != AiProvider::Custom {
        return Ok(());
    }
    let endpoint = custom_origin(settings).ok_or(AiFailure::BadEndpoint)?;
    match classify_endpoint(&endpoint) {
        EndpointClass::Invalid => return Err(AiFailure::BadEndpoint),
        EndpointClass::NeedsConfirmation if !insecure_ok => {
            return Err(AiFailure::InsecureNotConfirmed)
        }
        _ => {}
    }
    secrets
        .set(vault_id, SecretKind::AiCustomOrigin, &endpoint)
        .await
        .map_err(|_| AiFailure::SecretsUnavailable)
}

/// Свой адрес из настроек должен быть одобрен на этом устройстве: иначе адрес,
/// подменённый через синхронизацию, получил бы ключ и текст запросов (M1).
pub async fn require_approved<S: SecretStore>(
    secrets: &S,
    vault_id: &str,
    settings: &AiSettings,
) -> Result<(), AiFailure> {
    if settings.provider != AiProvider::Custom {
        return Ok(());
    }
    let approved = secrets
        .get(vault_id, SecretKind::AiCustomOrigin)
        .await
        .map_err(|_| AiFailure::SecretsUnavailable)?;
    if approved.is_some() && approved == custom_origin(settings) {
        Ok(())
    } else {
        Err(AiFailure::EndpointNotApproved)
    }
}

/// Шаг 2. Только сеть: vault здесь не нужен.
pub async fn send_probe<T: AiTransport>(
    transport: &T,
    prepared: &PreparedProbe,
    key: Option<&str>,
) -> Result<ProbeReply, AiFailure> {
    let mut request = prepared.request.clone();
    authorize(&mut request, prepared.provider, key);
    let response = transport.send(request).await?;
    check_status(response.status)?;
    parse_probe(prepared.provider, &response.body)
}

/// Шаг 3. Итог в журнал. Сбой этой записи итог не отменяет: запрос уже выполнен, а
/// запись останется в журнале как оборванная.
pub async fn finish_probe<B: VaultBackend>(
    backend: &mut B,
    prepared: &PreparedProbe,
    now: i64,
    outcome: Result<ProbeReply, AiFailure>,
) -> Result<ProbeResult, AiFailure> {
    match outcome {
        Ok(reply) => {
            let cost_usd = match (reply.input_tokens, reply.output_tokens) {
                (Some(input), Some(output)) if prepared.provider == AiProvider::Anthropic => {
                    estimate_usd(&prepared.model, input, output)
                }
                _ => None,
            };
            let _ = record_done(
                backend,
                &prepared.id,
                now,
                None,
                reply.input_tokens,
                reply.output_tokens,
                cost_usd,
            )
            .await;
            Ok(ProbeResult { reply, cost_usd })
        }
        Err(failure) => {
            let _ = record_done(backend, &prepared.id, now, Some(failure), None, None, None).await;
            Err(failure)
        }
    }
}

/// Запрос списка моделей. Текста пользователя в нём нет, поэтому в журнал он не идёт.
pub fn prepare_models(settings: &AiSettings, allow_custom: bool) -> Result<HttpRequest, AiFailure> {
    if !settings.enabled {
        return Err(AiFailure::Disabled);
    }
    Ok(models_request(
        settings.provider,
        &base_url(settings, allow_custom)?,
    ))
}

pub async fn send_models<T: AiTransport>(
    transport: &T,
    provider: AiProvider,
    mut request: HttpRequest,
    key: Option<&str>,
) -> Result<Vec<String>, AiFailure> {
    authorize(&mut request, provider, key);
    let response = transport.send(request).await?;
    check_status(response.status)?;
    parse_models(&response.body)
}
