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

/// Папка уроков по умолчанию (фича 051, FR-006).
pub const DEFAULT_LESSONS_FOLDER: &str = "Уроки";
/// Объём переписки (символы / 4), после которого она сжимается (фича 051, research §6).
pub const DEFAULT_CONTEXT_BUDGET: u32 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Папка, где создаются уроки (фича 051). Уроки, перенесённые в другое место,
    /// остаются уроками: список строится по всем файлам `.lesson`.
    pub lessons_folder: String,
    /// Порог сжатия переписки (фича 051).
    pub context_budget: u32,
    /// Пополнять граф знаний после хода учителя сам (фича 054, FR-003). Ядро только
    /// сообщает, что пора (`LessonView::knowledge_auto_due`); запрос запускает клиент.
    pub knowledge_auto: bool,
    /// Агент читает граф знаний инструментом `read_knowledge` (фича 054, FR-009).
    /// Выключено: инструмента нет в запросе, содержимое графа модели не уходит.
    pub knowledge_agent_reads: bool,
    /// Показывать вид «Граф знаний» (фича 054): настройка интерфейса, едет с vault.
    pub knowledge_map: bool,
}

impl Default for AiSettings {
    fn default() -> Self {
        AiSettings {
            enabled: false,
            provider: AiProvider::default(),
            custom_endpoint: None,
            insecure_confirmed_for: None,
            model: None,
            denied_folders: Vec::new(),
            lessons_folder: DEFAULT_LESSONS_FOLDER.to_owned(),
            context_budget: DEFAULT_CONTEXT_BUDGET,
            knowledge_auto: true,
            knowledge_agent_reads: false,
            knowledge_map: true,
        }
    }
}

/// Границы порога сжатия (ревью безопасности 051, M2): меньше — сжатие на каждом ходе,
/// больше — запросы, которые провайдер всё равно не примет.
pub const CONTEXT_BUDGET_MIN: u32 = 8_000;
pub const CONTEXT_BUDGET_MAX: u32 = 1_000_000;

/// Настройки едут с синхронизацией, то есть приходят и от других устройств: значения,
/// которые при сохранении исправляются, исправляются и при чтении (M2).
fn sanitize_lessons(settings: &mut AiSettings) {
    settings.lessons_folder = match normalize_folder(Path::new(settings.lessons_folder.trim())) {
        Some(folder) if !is_service_folder(&folder) => folder,
        _ => DEFAULT_LESSONS_FOLDER.to_owned(),
    };
    settings.context_budget = match settings.context_budget {
        0 => DEFAULT_CONTEXT_BUDGET,
        budget => budget.clamp(CONTEXT_BUDGET_MIN, CONTEXT_BUDGET_MAX),
    };
}

/// Служебная папка в любом написании, которое понимает самая снисходительная файловая
/// система: регистр, хвостовые точки и пробелы.
fn is_service_folder(folder: &str) -> bool {
    folder
        .split('/')
        .next()
        .map(|first| first.trim_end_matches(['.', ' ']).to_lowercase())
        .is_some_and(|first| first == service_dir::SERVICE_DIR_NAME.trim_end_matches(['.', ' ']))
}

pub async fn load_settings<B: VaultBackend>(backend: &B) -> AiSettings {
    let Ok(bytes) = backend.read_bytes(&service_dir::ai_settings_path()).await else {
        return AiSettings::default();
    };
    let mut settings: AiSettings = serde_json::from_slice(&bytes).unwrap_or_default();
    sanitize_lessons(&mut settings);
    settings
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
    // Уроки не могут лежать в служебном месте: оно закрыто для агента и синхронизации.
    sanitize_lessons(&mut next);
    if next.enabled && next.provider == AiProvider::Anthropic && next.model.is_none() {
        next.model = Some(DEFAULT_ANTHROPIC_MODEL.to_owned());
    }
    Ok(next)
}
