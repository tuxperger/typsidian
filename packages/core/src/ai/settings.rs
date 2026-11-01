//! Настройки обучения с ИИ: файл `.typsidian/ai.json`, едет с синхронизацией.
//!
//! Нет файла или он нечитаем — функция выключена: повреждение не должно
//! включать отправку данных (FR-001). Ключа здесь нет (FR-007).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::access::normalize_folder;
use super::endpoint::{classify_endpoint, EndpointClass};
use super::AiFailure;
use crate::sync::secrets::SecretKind;
use crate::vault::backend::VaultBackend;
use crate::vault::service_dir;

/// Модель при подключении Anthropic, пока пользователь не выбрал другую.
pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-opus-5-5";

/// Вид подключения. Внешние программы и вход через подписки не поддерживаются
/// (FR-004).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum AiProvider {
    #[default]
    Anthropic,
    OpenAi,
    Custom,
}

impl AiProvider {
    /// У каждого подключения свой ключ: ключ облака никогда не уйдёт на свой адрес,
    /// даже если адрес подменён через синхронизацию.
    pub fn secret_kind(self) -> SecretKind {
        match self {
            AiProvider::Anthropic => SecretKind::AiAnthropicKey,
            AiProvider::OpenAi => SecretKind::AiOpenAiKey,
            AiProvider::Custom => SecretKind::AiCustomKey,
        }
    }

    /// Ключ необязателен только у своего адреса: локальной модели он не нужен.
    pub fn requires_key(self) -> bool {
        !matches!(self, AiProvider::Custom)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(default)]
pub struct AiSettings {
    pub enabled: bool,
    pub provider: AiProvider,
    pub custom_endpoint: Option<String>,
    /// Адрес, для которого подтверждено незашифрованное соединение. Хранится сам
    /// адрес, а не флаг: смена адреса сбрасывает подтверждение (FR-010).
    pub insecure_confirmed_for: Option<String>,
    pub model: Option<String>,
    pub denied_folders: Vec<String>,
}

pub async fn load_settings<B: VaultBackend>(backend: &B) -> AiSettings {
    let Ok(bytes) = backend.read_bytes(&service_dir::ai_settings_path()).await else {
        return AiSettings::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// Пишет настройки; неверный адрес отклоняется, папки нормализуются.
pub async fn save_settings<B: VaultBackend>(
    backend: &mut B,
    settings: &AiSettings,
) -> Result<(), AiFailure> {
    let normalized = normalized(settings)?;
    let bytes = serde_json::to_vec_pretty(&normalized).map_err(|_| AiFailure::SettingsFailed)?;
    backend
        .write_bytes(&service_dir::ai_settings_path(), &bytes)
        .await
        .map_err(|_| AiFailure::SettingsFailed)
}

fn normalized(settings: &AiSettings) -> Result<AiSettings, AiFailure> {
    let mut next = settings.clone();
    if let Some(endpoint) = next.custom_endpoint.as_deref() {
        if classify_endpoint(endpoint) == EndpointClass::Invalid {
            return Err(AiFailure::BadEndpoint);
        }
    }
    if next.insecure_confirmed_for != next.custom_endpoint {
        next.insecure_confirmed_for = None;
    }
    let mut folders: Vec<String> = Vec::new();
    for folder in &settings.denied_folders {
        if let Some(clean) = normalize_folder(Path::new(folder)) {
            if !folders.contains(&clean) {
                folders.push(clean);
            }
        }
    }
    next.denied_folders = folders;
    if next.enabled && next.provider == AiProvider::Anthropic && next.model.is_none() {
        next.model = Some(DEFAULT_ANTHROPIC_MODEL.to_owned());
    }
    Ok(next)
}
