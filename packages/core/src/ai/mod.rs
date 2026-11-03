//! Провайдеры моделей для обучения с ИИ (фича 050, конституция, принцип IX).
//!
//! Настройки vault, проверка адреса и доступа к файлам, протоколы, журнал
//! отправленного и справочник цен. Запрос к модели идёт в три шага:
//! [`client::begin_probe`] пишет журнал, [`client::send_probe`] ходит в сеть,
//! [`client::finish_probe`] записывает итог. Сеть вынесена отдельно намеренно:
//! на время запроса не должны держаться ни блокировка vault, ни очередь вызовов
//! wasm, иначе медленная модель заморозила бы редактор (FR-017).

pub mod access;
pub mod cancel;
pub mod client;
pub mod endpoint;
pub mod journal;
pub mod knowledge;
pub mod lesson;
pub mod price;
pub mod protocol;
pub mod settings;
pub mod transport;
#[cfg(target_arch = "wasm32")]
pub mod web;

use serde::{Deserialize, Serialize};

/// Отказ обучения с ИИ. Без текста: сообщение строит UI из локалей, поэтому ни
/// ключ, ни ответ сервера сюда попасть не могут (FR-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum AiFailure {
    #[error("обучение с ИИ выключено")]
    Disabled,
    #[error("нет связи")]
    Offline,
    #[error("истёк предел ожидания")]
    Timeout,
    #[error("запрос отменён")]
    Cancelled,
    #[error("ключ не задан")]
    KeyMissing,
    #[error("модель не выбрана")]
    ModelMissing,
    #[error("провайдер отверг ключ")]
    BadKey,
    #[error("проблема с оплатой")]
    Billing,
    #[error("нет прав")]
    Forbidden,
    #[error("модель не найдена")]
    ModelNotFound,
    #[error("исчерпан лимит запросов")]
    RateLimited,
    #[error("провайдер недоступен")]
    ProviderDown,
    #[error("браузер не пустил запрос")]
    BlockedByBrowser,
    #[error("адрес перенаправил запрос")]
    Redirected,
    #[error("незашифрованный адрес не подтверждён")]
    InsecureNotConfirmed,
    #[error("неверный адрес")]
    BadEndpoint,
    #[error("свой адрес отключён на этом сервере")]
    CustomEndpointDisabled,
    #[error("адрес не одобрен на этом устройстве")]
    EndpointNotApproved,
    #[error("адрес ведёт во внутреннюю сеть сервера")]
    InternalAddress,
    #[error("неожиданный ответ провайдера")]
    BadResponse,
    #[error("не удалось записать журнал")]
    JournalFailed,
    #[error("не удалось записать настройки")]
    SettingsFailed,
    #[error("хранилище секретов недоступно")]
    SecretsUnavailable,
    /// Урок изменился с тех пор, как его видел интерфейс (фича 051, research §10).
    #[error("урок изменился")]
    LessonChanged,
    /// У файла урока нет читаемой шапки или формат новее известного.
    #[error("урок не читается")]
    LessonUnreadable,
    #[error("цель урока пуста")]
    GoalEmpty,
}
