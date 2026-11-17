//! Обучение с ИИ (фича 050) — общая таблица для обоих таргетов. Сети здесь нет:
//! запросы принимает подделка транспорта, и она же считает, сколько их было.

#![allow(dead_code)]

use std::cell::RefCell;
use std::path::Path;

use typsidian_core::ai::access::{check_access, move_denied_folders};
use typsidian_core::ai::cancel::{cancel, cancellable};
use typsidian_core::ai::client::{
    approve_endpoint, begin_probe, finish_probe, prepare_models, provider_key, require_approved,
    send_models, send_probe, Connection, PreparedProbe,
};
use typsidian_core::ai::endpoint::base_url;
use typsidian_core::ai::journal::{journal_clear, journal_entries, JournalState, MAX_ENTRIES};
use typsidian_core::ai::price::estimate_usd;
use typsidian_core::ai::protocol::{authorize, check_status, parse_probe, PROBE_TEXT};
use typsidian_core::ai::settings::{load_settings, save_settings, AiProvider, AiSettings};
use typsidian_core::ai::transport::{AiTransport, HttpMethod, HttpRequest, HttpResponse};
use typsidian_core::ai::AiFailure;
use typsidian_core::sync::secrets::{MemorySecretStore, SecretKind, SecretStore};
use typsidian_core::vault::service_dir;

use super::MemoryVault;

const JOURNAL: &str = ".typsidian/ai-journal.jsonl";
const KEY: &str = "sk-очень-секретный-ключ";

struct FakeTransport {
    sent: RefCell<Vec<HttpRequest>>,
    reply: Result<HttpResponse, AiFailure>,
}

impl FakeTransport {
    fn replying(status: u16, body: &str) -> Self {
        FakeTransport {
            sent: RefCell::default(),
            reply: Ok(HttpResponse {
                status,
                body: body.to_owned(),
            }),
        }
    }
}

impl AiTransport for FakeTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiFailure> {
        self.sent.borrow_mut().push(request);
        self.reply.clone()
    }
}

const ANTHROPIC_OK: &str = r#"{"content":[{"type":"thinking","thinking":""},
    {"type":"text","text":"готово"}],"usage":{"input_tokens":1000000,"output_tokens":1000000}}"#;
const OPENAI_OK: &str = r#"{"choices":[{"message":{"content":"готово"}}],
    "usage":{"prompt_tokens":3,"completion_tokens":1}}"#;

fn custom(endpoint: &str) -> AiSettings {
    AiSettings {
        enabled: true,
        provider: AiProvider::Custom,
        custom_endpoint: Some(endpoint.to_owned()),
        model: Some("своя".to_owned()),
        ..AiSettings::default()
    }
}

fn enabled(provider: AiProvider, model: &str) -> AiSettings {
    AiSettings {
        enabled: true,
        provider,
        model: Some(model.to_owned()),
        ..AiSettings::default()
    }
}

/// Начало пробного запроса с проверенным снимком текущих настроек, как у оболочки.
async fn begin(vault: &mut MemoryVault, id: &str, now: i64) -> Result<PreparedProbe, AiFailure> {
    let verified = Connection::verify(&load_settings(vault).await, true)?;
    begin_probe(vault, id, now, true, &verified).await
}

/// Отправка со снимком по самому запросу, как в wasm.
async fn send(
    transport: &FakeTransport,
    prepared: &PreparedProbe,
    key: Option<&str>,
) -> Result<typsidian_core::ai::protocol::ProbeReply, AiFailure> {
    let verified = Connection::of_request(prepared.provider, &prepared.request.url);
    send_probe(transport, prepared, key, &verified).await
}

async fn vault_with(settings: &AiSettings) -> MemoryVault {
    let mut vault = MemoryVault::new();
    save_settings(&mut vault, settings).await.unwrap();
    vault
}

pub async fn anthropic_gets_a_default_model() {
    let settings = AiSettings {
        enabled: true,
        ..AiSettings::default()
    };
    let vault = vault_with(&settings).await;
    assert_eq!(
        load_settings(&vault).await.model.as_deref(),
        Some("claude-opus-5-5")
    );
}

pub async fn missing_or_broken_settings_mean_disabled() {
    assert!(!load_settings(&MemoryVault::new()).await.enabled);
    let broken = MemoryVault::new().with_file(".typsidian/ai.json", "{ не json");
    assert!(!load_settings(&broken).await.enabled);
}

/// SC-002: выключено — ни запроса, ни записи в журнал.
pub async fn disabled_makes_no_requests() {
    let mut vault = MemoryVault::new();
    assert_eq!(
        begin(&mut vault, "1", 0).await.unwrap_err(),
        AiFailure::Disabled
    );
    assert_eq!(
        prepare_models(&AiSettings::default(), true).unwrap_err(),
        AiFailure::Disabled
    );
    assert!(vault.bytes_of(JOURNAL).is_none());
}

pub async fn journal_gets_request_before_sending_and_never_the_key() {
    let mut vault = vault_with(&enabled(AiProvider::Anthropic, "claude-opus-5-5")).await;
    let prepared = begin(&mut vault, "1", 100).await.unwrap();

    // До сети запись уже есть и показывается как оборванная.
    let entries = journal_entries(&vault).await;
    assert_eq!(entries[0].state, JournalState::Interrupted);
    assert!(entries[0].request.contains(PROBE_TEXT));

    let transport = FakeTransport::replying(200, ANTHROPIC_OK);
    let reply = send(&transport, &prepared, Some(KEY)).await;
    let result = finish_probe(&mut vault, &prepared, 200, reply)
        .await
        .unwrap();

    assert_eq!(result.reply.text, "готово");
    assert_eq!(result.cost_usd, Some(24.0));
    let sent = transport.sent.borrow();
    assert!(sent[0]
        .headers
        .iter()
        .any(|(n, v)| n == "x-api-key" && v == KEY));
    assert!(sent[0]
        .headers
        .iter()
        .any(|(n, _)| n == "anthropic-version"));

    let entries = journal_entries(&vault).await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].state, JournalState::Done);
    assert_eq!(entries[0].cost_usd, Some(24.0));
    let raw = String::from_utf8(vault.bytes_of(JOURNAL).unwrap()).unwrap();
    assert!(!raw.contains(KEY), "ключ попал в журнал");
}

/// FR-012: журнал не записался — запроса быть не должно.
pub async fn journal_failure_stops_the_request() {
    let settings = enabled(AiProvider::Anthropic, "claude-opus-5-5");
    let mut vault = vault_with(&settings).await.with_write_failure(JOURNAL);
    assert_eq!(
        begin(&mut vault, "1", 0).await.unwrap_err(),
        AiFailure::JournalFailed
    );
}

pub async fn unknown_price_is_not_zero() {
    let settings = AiSettings {
        custom_endpoint: Some("http://127.0.0.1:11434/v1".to_owned()),
        ..enabled(AiProvider::Custom, "qwen3:32b")
    };
    let mut vault = vault_with(&settings).await;
    let prepared = begin(&mut vault, "1", 0).await.unwrap();
    assert_eq!(
        prepared.request.url,
        "http://127.0.0.1:11434/v1/chat/completions"
    );

    let transport = FakeTransport::replying(200, OPENAI_OK);
    let reply = send(&transport, &prepared, None).await;
    let result = finish_probe(&mut vault, &prepared, 1, reply).await.unwrap();
    assert_eq!(result.reply.input_tokens, Some(3));
    assert_eq!(result.cost_usd, None);
    assert_eq!(estimate_usd("gpt-неизвестная", 10, 10), None);
    // Совпадение имени со справочником на чужом адресе цены не даёт.
    let settings = AiSettings {
        custom_endpoint: Some("http://127.0.0.1:1/v1".to_owned()),
        ..enabled(AiProvider::Custom, "claude-opus-5-5")
    };
    let mut vault = vault_with(&settings).await;
    let prepared = begin(&mut vault, "2", 0).await.unwrap();
    let reply = send(&transport, &prepared, None).await;
    assert_eq!(
        finish_probe(&mut vault, &prepared, 1, reply)
            .await
            .unwrap()
            .cost_usd,
        None
    );
}

pub async fn failure_is_recorded_and_carries_no_server_text() {
    let mut vault = vault_with(&enabled(AiProvider::Anthropic, "claude-opus-5-5")).await;
    let prepared = begin(&mut vault, "1", 0).await.unwrap();
    let transport = FakeTransport::replying(401, "invalid x-api-key sk-утечка");
    let reply = send(&transport, &prepared, Some(KEY)).await;
    assert_eq!(reply.clone().unwrap_err(), AiFailure::BadKey);
    assert_eq!(
        finish_probe(&mut vault, &prepared, 1, reply)
            .await
            .unwrap_err(),
        AiFailure::BadKey
    );
    let entries = journal_entries(&vault).await;
    assert_eq!(entries[0].state, JournalState::Failed);
    assert_eq!(entries[0].failure, Some(AiFailure::BadKey));
    let raw = String::from_utf8(vault.bytes_of(JOURNAL).unwrap()).unwrap();
    assert!(!raw.contains("утечка"));
}

pub async fn statuses_map_to_failures() {
    use AiFailure::*;
    for (status, failure) in [
        (301, Redirected),
        (307, Redirected),
        (401, BadKey),
        (402, Billing),
        (403, Forbidden),
        (404, ModelNotFound),
        (429, RateLimited),
        (500, ProviderDown),
        (529, ProviderDown),
    ] {
        assert_eq!(check_status(status), Err(failure), "{status}");
    }
    assert_eq!(check_status(200), Ok(()));
    assert_eq!(
        parse_probe(AiProvider::OpenAi, "<html>").unwrap_err(),
        BadResponse
    );
}

pub async fn journal_keeps_last_entries_and_clears() {
    let mut vault = vault_with(&enabled(AiProvider::Anthropic, "m")).await;
    for id in 0..MAX_ENTRIES + 3 {
        begin(&mut vault, &id.to_string(), id as i64).await.unwrap();
    }
    let entries = journal_entries(&vault).await;
    assert_eq!(entries.len(), MAX_ENTRIES);
    assert_eq!(entries[0].id, (MAX_ENTRIES + 2).to_string());
    assert_eq!(entries[MAX_ENTRIES - 1].id, "3");

    assert_eq!(journal_clear(&mut vault).await.unwrap(), MAX_ENTRIES as u32);
    assert!(journal_entries(&vault).await.is_empty());
}

pub async fn journal_stays_out_of_sync_and_settings_travel() {
    let derived = service_dir::derived_paths();
    assert!(derived.iter().any(|path| path == JOURNAL));
    assert!(!derived.iter().any(|path| path == ".typsidian/ai.json"));
}

pub async fn endpoints_are_guarded() {
    let custom = |endpoint: &str| AiSettings {
        custom_endpoint: Some(endpoint.to_owned()),
        ..enabled(AiProvider::Custom, "m")
    };
    // Сервер не разрешил свой адрес.
    assert_eq!(
        base_url(&custom("http://127.0.0.1/v1"), false),
        Err(AiFailure::CustomEndpointDisabled)
    );
    // Незашифрованно по сети — только с подтверждением именно этого адреса.
    let lan = "http://192.168.1.20/v1";
    assert_eq!(
        base_url(&custom(lan), true),
        Err(AiFailure::InsecureNotConfirmed)
    );
    let mut confirmed = custom(lan);
    confirmed.insecure_confirmed_for = Some("http://192.168.1.21/v1".to_owned());
    assert_eq!(
        base_url(&confirmed, true),
        Err(AiFailure::InsecureNotConfirmed)
    );
    confirmed.insecure_confirmed_for = Some(lan.to_owned());
    assert_eq!(base_url(&confirmed, true).unwrap(), lan);
    // Сохранение сбрасывает чужое подтверждение и отвергает плохой адрес.
    let mut vault = MemoryVault::new();
    let mut stale = custom(lan);
    stale.insecure_confirmed_for = Some("http://192.168.1.21/v1".to_owned());
    save_settings(&mut vault, &stale).await.unwrap();
    assert_eq!(load_settings(&vault).await.insecure_confirmed_for, None);
    assert_eq!(
        save_settings(&mut vault, &custom("https://a.org/?key=секрет")).await,
        Err(AiFailure::BadEndpoint)
    );
}

pub async fn cloud_key_never_goes_to_custom_endpoint() {
    let secrets = MemorySecretStore::new();
    secrets
        .set("v", SecretKind::AiAnthropicKey, KEY)
        .await
        .unwrap();
    assert_eq!(
        provider_key(&secrets, "v", AiProvider::Anthropic)
            .await
            .unwrap()
            .as_deref(),
        Some(KEY)
    );
    assert_eq!(
        provider_key(&secrets, "v", AiProvider::Custom)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        provider_key(&secrets, "v", AiProvider::OpenAi)
            .await
            .unwrap_err(),
        AiFailure::KeyMissing
    );
    // Другой vault ключа не видит.
    assert_eq!(
        provider_key(&secrets, "w", AiProvider::Anthropic)
            .await
            .unwrap_err(),
        AiFailure::KeyMissing
    );

    let mut request = HttpRequest {
        method: HttpMethod::Get,
        url: "http://127.0.0.1/v1/models".to_owned(),
        headers: Vec::new(),
        body: None,
    };
    authorize(&mut request, AiProvider::Custom, None);
    assert!(request.headers.is_empty());
}

pub async fn models_are_listed_sorted() {
    let settings = enabled(AiProvider::OpenAi, "m");
    let request = prepare_models(&settings, true).unwrap();
    let transport =
        FakeTransport::replying(200, r#"{"data":[{"id":"b"},{"id":"a"},{"name":"без id"}]}"#);
    let models = send_models(&transport, AiProvider::OpenAi, request, Some(KEY))
        .await
        .unwrap();
    assert_eq!(models, ["a", "b"]);
}

pub async fn access_stays_inside_the_vault() {
    let settings = AiSettings {
        denied_folders: vec!["Личное".to_owned(), "Работа/Клиенты".to_owned()],
        enabled: true,
        ..AiSettings::default()
    };
    for allowed in ["Заметка.typ", "Личное2/а.typ", "Работа/план.typ"] {
        assert!(check_access(allowed, &settings), "{allowed}");
    }
    for denied in [
        "Личное/а.typ",
        "Личное/глубже/а.typ",
        "Работа/Клиенты/а.typ",
        ".typsidian/ai.json",
        ".typsidian/ai-journal.jsonl",
        "../соседний/а.typ",
        "Заметки/../../а.typ",
        // Другое написание того же пути на снисходительной файловой системе.
        "личное/а.typ",
        "ЛИЧНОЕ/а.typ",
        "Личное./а.typ",
        "Личное /а.typ",
        "работа/клиенты/а.typ",
        ".TYPSIDIAN/ai-journal.jsonl",
        ".typsidian./ai.json",
        ".../а.typ",
        "/etc/passwd",
        "",
    ] {
        assert!(!check_access(denied, &settings), "{denied}");
    }
    // Выключенная функция не даёт доступа ни к чему.
    let disabled = AiSettings {
        enabled: false,
        ..settings.clone()
    };
    assert!(!check_access("Заметка.typ", &disabled));
}

/// «й» в форме NFD (и + кратка) — тот же путь, что и в NFC.
pub async fn access_ignores_unicode_form() {
    let settings = AiSettings {
        enabled: true,
        denied_folders: vec!["Мой".to_owned()],
        ..AiSettings::default()
    };
    assert!(!check_access("Мои\u{306}/а.typ", &settings));
    assert!(check_access("Мои/а.typ", &settings));
}

pub async fn denied_folder_follows_the_move() {
    let settings = AiSettings {
        enabled: true,
        denied_folders: vec!["Личное/Дневник".to_owned(), "Другое".to_owned()],
        ..AiSettings::default()
    };
    let mut vault = vault_with(&settings).await;
    move_denied_folders(&mut vault, Path::new("Личное"), Path::new("Архив/Личное")).await;
    let moved = load_settings(&vault).await;
    assert_eq!(moved.denied_folders, ["Архив/Личное/Дневник", "Другое"]);
    assert!(!check_access("Архив/Личное/Дневник/а.typ", &moved));
}

pub async fn cancel_stops_waiting_at_once() {
    let waiting = cancellable("v", "r", async {
        futures_util::future::pending::<Result<u32, AiFailure>>().await
    });
    futures_util::pin_mut!(waiting);
    assert!(futures_util::poll!(&mut waiting).is_pending());
    // Чужой vault запрос не отменяет.
    cancel("w", "r");
    assert!(futures_util::poll!(&mut waiting).is_pending());
    cancel("v", "r");
    assert_eq!(waiting.await, Err(AiFailure::Cancelled));
}

/// Повтор id занятого запроса отказывает сразу и не отнимает у первого отмену
/// (ревью безопасности 050, L3).
pub async fn duplicate_request_id_keeps_the_first_cancellable() {
    let first = cancellable("v", "dup", async {
        futures_util::future::pending::<Result<u32, AiFailure>>().await
    });
    futures_util::pin_mut!(first);
    assert!(futures_util::poll!(&mut first).is_pending());
    let second = cancellable("v", "dup", async { Ok(7u32) }).await;
    assert_eq!(second, Err(AiFailure::Cancelled));
    assert!(futures_util::poll!(&mut first).is_pending());
    cancel("v", "dup");
    assert_eq!(first.await, Err(AiFailure::Cancelled));
}

/// Свой адрес, пришедший с синхронизацией, не получает запросов, пока его не
/// одобрили на этом устройстве; пришедшее с ним подтверждение незашифрованного
/// соединения не в счёт (ревью безопасности 050, M1).
pub async fn custom_endpoint_needs_approval_on_this_device() {
    let secrets = MemorySecretStore::new();
    let mut settings = AiSettings {
        enabled: true,
        provider: AiProvider::Custom,
        custom_endpoint: Some("https://models.example.org/v1/".to_owned()),
        model: Some("m".to_owned()),
        ..AiSettings::default()
    };
    // Облачные подключения одобрения не требуют: их адрес не настраивается.
    let cloud = AiSettings {
        provider: AiProvider::Anthropic,
        ..settings.clone()
    };
    assert_eq!(require_approved(&secrets, "v", &cloud).await, Ok(()));

    assert_eq!(
        require_approved(&secrets, "v", &settings).await,
        Err(AiFailure::EndpointNotApproved)
    );
    approve_endpoint(&secrets, "v", &settings, false)
        .await
        .unwrap();
    assert_eq!(require_approved(&secrets, "v", &settings).await, Ok(()));
    // Одобрение — у этого vault, не у соседнего.
    assert_eq!(
        require_approved(&secrets, "w", &settings).await,
        Err(AiFailure::EndpointNotApproved)
    );

    // Другое устройство поменяло адрес и «подтвердило» его в общих настройках.
    settings.custom_endpoint = Some("http://203.0.113.9/v1".to_owned());
    settings.insecure_confirmed_for = settings.custom_endpoint.clone();
    assert_eq!(
        require_approved(&secrets, "v", &settings).await,
        Err(AiFailure::EndpointNotApproved)
    );
    assert_eq!(
        approve_endpoint(&secrets, "v", &settings, false).await,
        Err(AiFailure::InsecureNotConfirmed)
    );
    assert_eq!(
        require_approved(&secrets, "v", &settings).await,
        Err(AiFailure::EndpointNotApproved)
    );
    approve_endpoint(&secrets, "v", &settings, true)
        .await
        .unwrap();
    assert_eq!(require_approved(&secrets, "v", &settings).await, Ok(()));
}

/// H1 для пробного запроса 050: настройки сменились после проверки оболочки.
pub async fn probe_refuses_a_connection_that_changed_after_the_check() {
    let mut vault = vault_with(&custom("https://models.example.org/v1")).await;
    let verified = Connection::verify(&load_settings(&vault).await, true).unwrap();
    save_settings(&mut vault, &custom("https://evil.example.net/v1"))
        .await
        .unwrap();

    // Построение запроса: отказ раньше записи в журнал.
    assert_eq!(
        begin_probe(&mut vault, "1", 0, true, &verified)
            .await
            .unwrap_err(),
        AiFailure::EndpointNotApproved
    );
    assert!(vault.bytes_of(JOURNAL).is_none());

    // Отправка: запрос на новый адрес со старым снимком ключ не получает.
    let moved = begin(&mut vault, "2", 0).await.unwrap();
    let transport = FakeTransport::replying(200, ANTHROPIC_OK);
    assert_eq!(
        send_probe(&transport, &moved, Some(KEY), &verified)
            .await
            .unwrap_err(),
        AiFailure::EndpointNotApproved
    );
    assert!(transport.sent.borrow().is_empty());
}

/// Журнал ограничен и в байтах (ревью 056, M2): старые запросы вытесняются целиком, последний
/// и тела оставшихся остаются дословными.
pub async fn journal_is_bounded_in_bytes() {
    use typsidian_core::ai::journal::MAX_BYTES;
    let mut vault = vault_with(&enabled(AiProvider::Anthropic, "m")).await;
    let body = |tag: char| tag.to_string().repeat(MAX_BYTES / 4);
    let mut text = String::new();
    for (index, tag) in ['a', 'b', 'c', 'd'].into_iter().enumerate() {
        let sent = serde_json::json!({
            "line": "Sent", "id": format!("big-{tag}"), "at": index, "provider": AiProvider::Anthropic,
            "model": "m", "endpoint": "https://api.anthropic.com/v1/messages", "request": body(tag),
        });
        let done = serde_json::json!({
            "line": "Done", "id": format!("big-{tag}"), "at": index, "failure": null,
            "input_tokens": null, "output_tokens": null, "cost_usd": null,
        });
        text.push_str(&format!("{sent}\n{done}\n"));
    }
    assert!(text.len() > MAX_BYTES, "заготовка больше предела");
    vault.replace_externally(JOURNAL, text);
    begin(&mut vault, "fresh", 9).await.unwrap();
    let entries = journal_entries(&vault).await;
    let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, ["fresh", "big-d", "big-c", "big-b"], "самая старая ушла целиком");
    assert_eq!(entries[1].request, body('d'), "тела дословны");
    assert_eq!(entries[1].state, JournalState::Done);
    let size = vault.bytes_of(JOURNAL).unwrap().len();
    assert!(size <= MAX_BYTES, "файл в пределе: {size}");
    // Один запрос больше предела не вытесняет сам себя.
    let mut huge = String::new();
    huge.push_str(&format!(
        "{}\n",
        serde_json::json!({
            "line": "Sent", "id": "huge", "at": 1, "provider": AiProvider::Anthropic, "model": "m",
            "endpoint": "https://api.anthropic.com/v1/messages", "request": "x".repeat(MAX_BYTES + 10),
        })
    ));
    vault.replace_externally(JOURNAL, huge);
    begin(&mut vault, "after-huge", 10).await.unwrap();
    let ids: Vec<String> = journal_entries(&vault)
        .await
        .into_iter()
        .map(|entry| entry.id)
        .collect();
    assert_eq!(ids, ["after-huge"]);
}
