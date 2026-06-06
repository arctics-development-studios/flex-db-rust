/// Every API error code returned by Flex DB server 2.6.x.
///
/// Match on these variants rather than the error message strings — messages
/// may change between server releases, but codes are stable.
///
/// # Example
///
/// ```rust,no_run
/// use flex_db::{ApiErrorCode, Error};
///
/// # async fn example(ns: flex_db::Namespace) -> Result<(), flex_db::Error> {
/// match ns.get::<serde_json::Value>("my-key").await {
///     Ok(obj) => println!("{:?}", obj.data),
///     Err(Error::Api { code: ApiErrorCode::NotFound, .. }) => println!("not found"),
///     Err(Error::Api { code: ApiErrorCode::RateLimitSecond, .. }) => { /* back off */ }
///     Err(e) => eprintln!("other: {e}"),
/// }
/// # Ok(()) }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiErrorCode {
    /// `ERR_MISSING_AUTH` (401) — no `Authorization` header was sent.
    #[error("Missing authorization header")]
    MissingAuth,

    /// `ERR_UNAUTHORIZED` (401) — token is invalid, expired, or revoked.
    #[error("Invalid or expired token")]
    Unauthorized,

    /// `ERR_PERMISSION_DENIED` (403) — token lacks the required permission bit
    /// (READ or WRITE) for this operation.
    #[error("Token does not have permission for this operation")]
    PermissionDenied,

    /// `ERR_NOT_FOUND` (404) — object does not exist at any storage tier.
    #[error("Object not found")]
    NotFound,

    /// `ERR_MISSING_FILTER` (400) — search called with an empty `filters` array.
    #[error("Missing required filter parameters")]
    MissingFilter,

    /// `ERR_INVALID_KEY` (400) — a caller-supplied key fails the nanoid constraint
    /// (alphabet `A-Za-z0-9_-`, length 5–21).
    #[error("Key is not valid (must be 5–21 chars, alphabet A-Za-z0-9_-)")]
    InvalidKey,

    /// `ERR_RATE_LIMIT_SECOND` (429) — per-second RPS cap exceeded.
    /// Back off and retry; do **not** retry immediately.
    #[error("Rate limit exceeded (per second)")]
    RateLimitSecond,

    /// `ERR_RATE_LIMIT_MONTH` (429) — monthly budget exhausted.
    /// Surface this to the caller; retrying will not help until the next billing cycle.
    #[error("Rate limit exceeded (monthly budget exhausted)")]
    RateLimitMonth,

    /// `ERR_REQUEST_TOO_LARGE` (413) — single object `data` exceeds the deployment's
    /// `cold_max_size` limit (typically 5 MB).
    #[error("Request body exceeds maximum object size")]
    RequestTooLarge,

    /// `ERR_BULK_TOO_LARGE` (413) — bulk request item count exceeds the operation's
    /// per-deployment limit.
    #[error("Bulk request exceeds maximum item limit")]
    BulkTooLarge,

    /// `ERR_UNPROCESSABLE_ENTITY` (422) — request body is not valid JSON or has
    /// the wrong shape.
    #[error("Request body is not valid JSON or has an unexpected shape")]
    UnprocessableEntity,

    /// `ERR_INTERNAL` (500) — unexpected server error.
    #[error("Internal server error")]
    Internal,

    /// The server returned a code string not recognized by this SDK version.
    /// Upgrade the SDK if this appears frequently.
    #[error("Unknown error code: {0}")]
    Unknown(String),
}

pub(crate) fn code_from_str(s: &str) -> ApiErrorCode {
    match s {
        "ERR_MISSING_AUTH"          => ApiErrorCode::MissingAuth,
        "ERR_UNAUTHORIZED"          => ApiErrorCode::Unauthorized,
        "ERR_PERMISSION_DENIED"     => ApiErrorCode::PermissionDenied,
        "ERR_NOT_FOUND"             => ApiErrorCode::NotFound,
        "ERR_MISSING_FILTER"        => ApiErrorCode::MissingFilter,
        "ERR_INVALID_KEY"           => ApiErrorCode::InvalidKey,
        "ERR_RATE_LIMIT_SECOND"     => ApiErrorCode::RateLimitSecond,
        "ERR_RATE_LIMIT_MONTH"      => ApiErrorCode::RateLimitMonth,
        "ERR_REQUEST_TOO_LARGE"     => ApiErrorCode::RequestTooLarge,
        "ERR_BULK_TOO_LARGE"        => ApiErrorCode::BulkTooLarge,
        "ERR_UNPROCESSABLE_ENTITY"  => ApiErrorCode::UnprocessableEntity,
        "ERR_INTERNAL"              => ApiErrorCode::Internal,
        other                       => ApiErrorCode::Unknown(other.to_owned()),
    }
}

/// Top-level SDK error type returned by every async method.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The API returned `ok: false` with a structured error body.
    ///
    /// Always match on [`ApiErrorCode`] variants rather than the message string.
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
