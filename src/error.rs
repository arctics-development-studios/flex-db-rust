/// Every API error code from Section 11 of the spec.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiErrorCode {
    #[error("Missing authorization header")]
    MissingAuth,
    #[error("Invalid or expired token")]
    Unauthorized,
    #[error("Missing X-Namespace header")]
    MissingNamespace,
    #[error("Token does not have permission for this operation")]
    PermissionDenied,
    #[error("Access forbidden")]
    Forbidden,
    #[error("Object not found")]
    NotFound,
    #[error("Missing required filter parameters")]
    MissingFilter,
    #[error("Rate limit exceeded (per second)")]
    RateLimitSecond,
    #[error("Rate limit exceeded (monthly)")]
    RateLimitMonth,
    #[error("Request body exceeds maximum size")]
    RequestTooLarge,
    #[error("Bulk request exceeds maximum item limit")]
    BulkTooLarge,
    #[error("Invalid request parameter")]
    InvalidRequest,
    #[error("Failed to store object")]
    StoreFailed,
    #[error("Failed to delete object")]
    DeleteFailed,
    #[error("Internal server error")]
    Internal,
    /// Server returned a code string not recognized by this SDK version.
    #[error("Unknown error code: {0}")]
    Unknown(String),
}

pub(crate) fn code_from_str(s: &str) -> ApiErrorCode {
    match s {
        "ERR_MISSING_AUTH"       => ApiErrorCode::MissingAuth,
        "ERR_UNAUTHORIZED"       => ApiErrorCode::Unauthorized,
        "ERR_MISSING_NAMESPACE"  => ApiErrorCode::MissingNamespace,
        "ERR_PERMISSION_DENIED"  => ApiErrorCode::PermissionDenied,
        "ERR_FORBIDDEN"          => ApiErrorCode::Forbidden,
        "ERR_NOT_FOUND"          => ApiErrorCode::NotFound,
        "ERR_MISSING_FILTER"     => ApiErrorCode::MissingFilter,
        "ERR_RATE_LIMIT_SECOND"  => ApiErrorCode::RateLimitSecond,
        "ERR_RATE_LIMIT_MONTH"   => ApiErrorCode::RateLimitMonth,
        "ERR_REQUEST_TOO_LARGE"  => ApiErrorCode::RequestTooLarge,
        "ERR_BULK_TOO_LARGE"     => ApiErrorCode::BulkTooLarge,
        "ERR_INVALID_REQUEST"    => ApiErrorCode::InvalidRequest,
        "ERR_STORE_FAILED"       => ApiErrorCode::StoreFailed,
        "ERR_DELETE_FAILED"      => ApiErrorCode::DeleteFailed,
        "ERR_INTERNAL"           => ApiErrorCode::Internal,
        other                    => ApiErrorCode::Unknown(other.to_owned()),
    }
}

/// Top-level SDK error type returned by all async methods.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The API returned `ok: false` with a structured error body.
    #[error("API error ({code}): {message}")]
    Api {
        code: ApiErrorCode,
        message: String,
    },

    /// HTTP transport error — connection refused, timeout, TLS failure, etc.
    #[error("HTTP transport error: {0}")]
    Transport(#[from] reqwest::Error),

    /// Response body could not be deserialized into the expected type.
    #[error("Deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;
