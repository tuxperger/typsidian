//! Отказы API (contracts/server-api.md): размеченное объединение `{"kind": …}` с кодом
//! HTTP. Внутренняя ошибка уходит в журнал, клиенту — только её вид.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    /// Нет сессии или она истекла.
    Unauthenticated,
    /// Неверный логин или пароль — без уточнения, что именно (US1 AS2).
    BadCredentials,
    /// Действие запрещено (нет CSRF-токена, команда вне белого списка).
    Forbidden,
    /// Нет такого vault у этого аккаунта — чужой выглядит так же (FR-004).
    NoVault,
    NotFound,
    TooManyAttempts {
        retry_after_s: u64,
    },
    Invalid {
        field: &'static str,
        reason: &'static str,
    },
    /// Пользователь провайдера входа не сопоставлен аккаунту (US3 AS3).
    UnknownAccount,
    Internal(anyhow::Error),
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        ApiError::Internal(error)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(error: std::io::Error) -> Self {
        ApiError::Internal(error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            ApiError::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                json!({ "kind": "Unauthenticated" }),
            ),
            ApiError::BadCredentials => (
                StatusCode::UNAUTHORIZED,
                json!({ "kind": "BadCredentials" }),
            ),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, json!({ "kind": "Forbidden" })),
            ApiError::NoVault => (StatusCode::NOT_FOUND, json!({ "kind": "NoVault" })),
            ApiError::NotFound => (StatusCode::NOT_FOUND, json!({ "kind": "NotFound" })),
            ApiError::TooManyAttempts { retry_after_s } => (
                StatusCode::TOO_MANY_REQUESTS,
                json!({ "kind": "TooManyAttempts", "retry_after_s": retry_after_s }),
            ),
            ApiError::Invalid { field, reason } => (
                StatusCode::BAD_REQUEST,
                json!({ "kind": "Invalid", "field": field, "reason": reason }),
            ),
            ApiError::UnknownAccount => {
                (StatusCode::FORBIDDEN, json!({ "kind": "UnknownAccount" }))
            }
            ApiError::Internal(error) => {
                tracing::error!("внутренняя ошибка: {error:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({ "kind": "Internal" }),
                )
            }
        };
        (status, Json(body)).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
