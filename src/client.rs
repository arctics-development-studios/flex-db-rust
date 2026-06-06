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
    pub(crate) fn auth(&self, req: RequestBuilder) -> RequestBuilder {
        req.header("Authorization", format!("Bearer {}", self.token))
    }
}

// ---------------------------------------------------------------------------
// Public client
// ---------------------------------------------------------------------------

/// Top-level Flex DB client.
///
/// Create one instance per application and clone freely — the internal HTTP
/// connection pool and credentials are wrapped in `Arc`, so cloning is cheap.
///
/// # Example
///
/// ```rust,no_run
/// use flex_db::FlexDb;
///
/// let client = FlexDb::new("https://api.flexdb.io", "your-jwt-token");
/// let ns = client.namespace("users");
/// ```
#[derive(Debug, Clone)]
pub struct FlexDb {
    pub(crate) inner: Arc<FlexDbInner>,
}

impl FlexDb {
    /// Create a new client.
    ///
    /// - `base_url` — scheme + host, no trailing slash (`"https://api.flexdb.io"`).
    /// - `token` — raw RS256 JWT issued by Flex DB, **without** the `Bearer ` prefix.
    ///   The token encodes a `db_id` and a 2-bit `perms` field (1=READ, 2=WRITE, 3=ALL).
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

    /// Return a [`Namespace`] scoped to the given namespace string.
    ///
    /// The returned `Namespace` shares the same underlying connection pool and
    /// credentials. Calling this method is a zero-cost string allocation.
    ///
    /// Namespaces are implicit — they spring into existence on first write and
    /// disappear when all their keys are deleted.
    pub fn namespace(&self, namespace: impl Into<String>) -> Namespace {
        Namespace {
            inner: Arc::clone(&self.inner),
            namespace: namespace.into(),
        }
    }

    /// `GET /health` — server liveness check. No authentication required.
    ///
    /// Returns `Ok` when the server is reachable and healthy.
    pub async fn health(&self) -> Result<HealthResponse> {
        let url = format!("{}/health", self.inner.base_url);
        let resp = self.inner.http.get(&url).send().await?;
        parse_raw(resp).await
    }
}

// ---------------------------------------------------------------------------
// Response parsers
// ---------------------------------------------------------------------------

/// Parses the standard Flex DB v2 response envelope:
/// `{ "v": "...", "ok": bool, "data": { ... } }`.
///
/// On `ok: true`  → deserializes the inner `data` object into `T`.
/// On `ok: false` → returns `Error::Api` with the structured error code.
pub(crate) async fn parse_response<T: DeserializeOwned>(resp: Response) -> Result<T> {
    let body: Value = resp.json().await?;

    match body["ok"].as_bool() {
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
        _ => Ok(serde_json::from_value(body["data"].clone())?),
    }
}

/// Parses a raw JSON response without the Flex DB envelope (used for `/health`).
async fn parse_raw<T: DeserializeOwned>(resp: Response) -> Result<T> {
    let body: Value = resp.json().await?;
    Ok(serde_json::from_value(body)?)
}
