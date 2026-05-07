use std::sync::Arc;

use reqwest::{RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::{code_from_str, Error, Result};
use crate::types::HealthResponse;
use crate::Namespace;

// ---------------------------------------------------------------------------
// Internal shared state
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(crate) struct FlexDbInner {
    pub(crate) base_url: String,
    pub(crate) token: String,
    pub(crate) http: reqwest::Client,
}

impl FlexDbInner {
    /// Attaches the Authorization header to any request builder.
    pub(crate) fn auth(&self, req: RequestBuilder) -> RequestBuilder {
        req.header("Authorization", format!("Bearer {}", self.token))
    }
}

// ---------------------------------------------------------------------------
// Public client
// ---------------------------------------------------------------------------

/// Top-level Flex DB client.
///
/// Construct once and clone freely — the internal HTTP client and credentials
/// are wrapped in `Arc`, so cloning is cheap and all clones share the same
/// connection pool.
///
/// # Example
///
/// ```rust,no_run
/// use flex_db::FlexDb;
///
/// let client = FlexDb::new("https://api.example.com", "your-jwt-token");
/// let ns = client.namespace("users");
/// ```
#[derive(Debug, Clone)]
pub struct FlexDb {
    pub(crate) inner: Arc<FlexDbInner>,
}

impl FlexDb {
    /// Create a new client.
    ///
    /// - `base_url`: scheme + host with no trailing slash, e.g. `"https://api.flexdb.io"`.
    /// - `token`: raw JWT (without the `Bearer ` prefix).
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        let http = reqwest::Client::new();
        Self {
            inner: Arc::new(FlexDbInner {
                base_url: base_url.into().trim_end_matches('/').to_owned(),
                token: token.into(),
                http,
            }),
        }
    }

    /// Return a [`Namespace`] scoped client for all data operations.
    ///
    /// The returned `Namespace` shares the same underlying HTTP client and
    /// credentials — calling `namespace()` is a zero-cost string allocation.
    pub fn namespace(&self, namespace: impl Into<String>) -> Namespace {
        Namespace {
            inner: Arc::clone(&self.inner),
            namespace: namespace.into(),
        }
    }

    /// `GET /health` — no authentication required.
    pub async fn health(&self) -> Result<HealthResponse> {
        let url = format!("{}/health", self.inner.base_url);
        let resp = self.inner.http.get(&url).send().await?;
        parse_response(resp).await
    }
}

// ---------------------------------------------------------------------------
// Shared response parser
// ---------------------------------------------------------------------------

/// Deserializes the Flex DB response envelope.
///
/// On `ok: false` → `Error::Api { code, message }`.
/// On `ok: true`  → deserializes the full JSON value into `T` (the `v` and
/// `ok` fields are silently ignored by serde because none of the response
/// types use `deny_unknown_fields`).
pub(crate) async fn parse_response<T: DeserializeOwned>(resp: Response) -> Result<T> {
    let body: Value = resp.json().await?;

    match body["ok"].as_bool() {
        Some(true) | None => Ok(serde_json::from_value(body)?),
        Some(false) => {
            let code = body["error"]["code"]
                .as_str()
                .map(code_from_str)
                .unwrap_or_else(|| code_from_str("ERR_INTERNAL"));
            let message = body["error"]["message"]
                .as_str()
                .unwrap_or("unknown error")
                .to_owned();
            Err(Error::Api { code, message })
        }
    }
}
