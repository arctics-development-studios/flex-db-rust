use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

use crate::client::{parse_response, FlexDbInner};
use crate::error::Result;
use crate::filter::SearchFilter;
use crate::types::{
    BulkCreateBody, BulkCreateItem, BulkCreateResult, BulkDeleteBody, BulkGetBody, BulkGetResponse,
    BulkSetBody, BulkSetItem, BulkWriteResponse, GetResponse, HydratedItem, ListPage, ListPageFull,
    SearchBody, WriteBody,
};

// ---------------------------------------------------------------------------
// Namespace client
// ---------------------------------------------------------------------------

/// A data client scoped to a specific namespace within a Flex DB database.
///
/// Obtain via [`FlexDb::namespace`](crate::FlexDb::namespace).
///
/// # Namespaces
///
/// A namespace (`ns`) organises keys within a database — think "collection" or
/// "table". It is not validated by the server beyond being a non-empty path
/// segment. Namespaces are implicit: they spring into existence on first write
/// and disappear when all their keys are deleted.
///
/// # Concurrency
///
/// `Namespace` is `Clone + Send + Sync`. All methods take `&self`, so you can
/// issue concurrent requests with `tokio::join!` or any async combinator
/// without cloning:
///
/// ```rust,no_run
/// # async fn example(ns: flex_db::Namespace) -> Result<(), flex_db::Error> {
/// let (a, b) = tokio::join!(
///     ns.get::<serde_json::Value>("key1"),
///     ns.get::<serde_json::Value>("key2"),
/// );
/// # Ok(()) }
/// ```
///
/// # Write visibility
///
/// Writes land in L2 (Valkey cache) immediately and are readable via `get`
/// right away. However, they do **not** appear in `list` or `search` results
/// until the write-buffer flush job commits them to DynamoDB (~60 s lag).
/// This is by design — document it to your users, not as a bug.
#[derive(Debug, Clone)]
pub struct Namespace {
    pub(crate) inner: Arc<FlexDbInner>,
    pub(crate) namespace: String,
}

impl Namespace {
    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.inner.base_url, path)
    }

    fn req_get(&self, path: &str) -> reqwest::RequestBuilder {
        self.inner.auth(self.inner.http.get(self.url(path)))
    }

    fn req_post(&self, path: &str) -> reqwest::RequestBuilder {
        self.inner.auth(self.inner.http.post(self.url(path)))
    }

    fn req_put(&self, path: &str) -> reqwest::RequestBuilder {
        self.inner.auth(self.inner.http.put(self.url(path)))
    }

    fn req_delete(&self, path: &str) -> reqwest::RequestBuilder {
        self.inner.auth(self.inner.http.delete(self.url(path)))
    }

    // -----------------------------------------------------------------------
    // Single-object operations
    // -----------------------------------------------------------------------

    /// `POST /v2/o/:ns` — create an object with a server-generated key.
    ///
    /// Returns the new key — a nanoid(21) string in the alphabet `A-Za-z0-9_-`.
    ///
    /// The object is immediately readable via [`get`](Self::get) but will not
    /// appear in [`list`](Self::list) or [`search`](Self::search) results for
    /// up to ~60 seconds while the write buffer flushes.
    ///
    /// # Arguments
    ///
    /// - `data` — any `serde::Serialize` value. Maximum serialized size is
    ///   deployment-specific (typically 5 MB).
    /// - `sp` — optional search properties stored with the object. Keys follow
    ///   `A-Za-z0-9_` (max 64 chars); values are JSON scalars. These are
    ///   **not** returned by `get` — they exist only to support `search` filters.
    pub async fn create<T: Serialize>(
        &self,
        data: &T,
        sp: Option<&Map<String, Value>>,
    ) -> Result<String> {
        let body = WriteBody { data, sp };
        let path = format!("/v2/o/{}", self.namespace);
        let resp = self.req_post(&path).json(&body).send().await?;
        let val: Value = parse_response(resp).await?;
        Ok(val["id"].as_str().unwrap_or_default().to_owned())
    }

    /// `GET /v2/o/:ns/:key` — retrieve a single object.
    ///
    /// Reads through the tier waterfall L1 → L2 → KV → Cold and returns the
    /// first hit. Returns `Error::Api { code: ApiErrorCode::NotFound, .. }` if
    /// no value is found at any tier.
    ///
    /// Note: `sp` (search properties) are **not** returned here. See [`search`](Self::search).
    ///
    /// # Key constraint
    ///
    /// Caller-supplied keys must use alphabet `A-Za-z0-9_-` and be 5–21 chars.
    /// Violation returns `Error::Api { code: ApiErrorCode::InvalidKey, .. }`.
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<GetResponse<T>> {
        let path = format!("/v2/o/{}/{}", self.namespace, key);
        let resp = self.req_get(&path).send().await?;
        parse_response(resp).await
    }

    /// `PUT /v2/o/:ns/:key` — create or fully replace an object at a known key.
    ///
    /// If the key already exists it is overwritten entirely. Tier assignment
    /// is recalculated on every write based on the new payload size.
    ///
    /// The object is immediately readable via `get` but will not appear in
    /// `list` or `search` for up to ~60 seconds.
    ///
    /// # Arguments
    ///
    /// - `key` — must pass the nanoid constraint: `A-Za-z0-9_-`, length 5–21.
    /// - `data` — any `serde::Serialize` value.
    /// - `sp` — optional search properties (see [`create`](Self::create)).
    pub async fn set<T: Serialize>(
        &self,
        key: &str,
        data: &T,
        sp: Option<&Map<String, Value>>,
    ) -> Result<()> {
        let body = WriteBody { data, sp };
        let path = format!("/v2/o/{}/{}", self.namespace, key);
        let resp = self.req_put(&path).json(&body).send().await?;
        parse_response::<Value>(resp).await?;
        Ok(())
    }

    /// `DELETE /v2/o/:ns/:key` — delete an object from all storage tiers.
    ///
    /// Performs a parallel delete across L2 data, L2 meta, KV, and S3.
    /// Also deregisters the key from the write buffer (handles objects that
    /// have not yet flushed).
    ///
    /// Always returns `Ok(())` even when the key does not exist (idempotent).
    pub async fn delete(&self, key: &str) -> Result<()> {
        let path = format!("/v2/o/{}/{}", self.namespace, key);
        let resp = self.req_delete(&path).send().await?;
        parse_response::<Value>(resp).await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // List operations
    // -----------------------------------------------------------------------

    /// `GET /v2/list/:ns` — fetch one page of object keys (non-hydrated).
    ///
    /// Only objects that have been flushed from the write buffer appear here.
    /// Objects written within the last ~60 seconds may be absent.
    ///
    /// # Arguments
    ///
    /// - `limit` — max keys to return (1–1000, server default 50).
    /// - `cursor` — opaque token from a previous response; `None` starts from
    ///   the beginning. A `None` cursor in the response means no more pages.
    pub async fn list(&self, limit: Option<u32>, cursor: Option<&str>) -> Result<ListPage> {
        let path = format!("/v2/list/{}", self.namespace);
        let mut req = self.req_get(&path);
        if let Some(l) = limit {
            req = req.query(&[("limit", l.to_string())]);
        }
        if let Some(c) = cursor {
            req = req.query(&[("cursor", c)]);
        }
        let resp = req.send().await?;
        parse_response(resp).await
    }

    /// `GET /v2/list/:ns?hydrate=true` — fetch one page of objects with their data.
    ///
    /// Runs a `bulk_get` waterfall after the DynamoDB query — values may be
    /// served from any tier. Same ~60 s write-buffer visibility lag as `list`.
    pub async fn list_full<T: DeserializeOwned>(
        &self,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ListPageFull<T>> {
        let path = format!("/v2/list/{}", self.namespace);
        let mut req = self.req_get(&path).query(&[("hydrate", "true")]);
        if let Some(l) = limit {
            req = req.query(&[("limit", l.to_string())]);
        }
        if let Some(c) = cursor {
            req = req.query(&[("cursor", c)]);
        }
        let resp = req.send().await?;
        parse_response(resp).await
    }

    /// Collect **all** object keys in the namespace across every page (non-hydrated).
    ///
    /// Pages are fetched sequentially; each cursor depends on the previous page.
    /// `page_size` sets the per-page `limit` (server default 50, max 1000).
    pub async fn list_all(&self, page_size: Option<u32>) -> Result<Vec<String>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let page = self.list(page_size, cursor.as_deref()).await?;
            all.extend(page.keys);
            match page.cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        Ok(all)
    }

    /// Collect **all** hydrated objects in the namespace across every page.
    pub async fn list_all_full<T: DeserializeOwned>(
        &self,
        page_size: Option<u32>,
    ) -> Result<Vec<HydratedItem<T>>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let page = self.list_full::<T>(page_size, cursor.as_deref()).await?;
            all.extend(page.items);
            match page.cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        Ok(all)
    }

    // -----------------------------------------------------------------------
    // Search operations
    // -----------------------------------------------------------------------

    /// `POST /v2/search/:ns` — fetch one page of keys matching all `filters` (AND logic).
    ///
    /// Queries the `sp` (search properties) sub-map stored with each object.
    /// `filters` must be non-empty; an empty slice returns
    /// `Error::Api { code: ApiErrorCode::MissingFilter, .. }`.
    ///
    /// Same ~60 s write-buffer visibility lag as `list` — only flushed objects
    /// appear in search results.
    ///
    /// # Arguments
    ///
    /// - `limit` — max results per page (1–1000, server default 50).
    /// - `cursor` — opaque pagination token from a previous response.
    pub async fn search(
        &self,
        filters: &[SearchFilter],
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ListPage> {
        let body = SearchBody { filters, limit, cursor, hydrate: false };
        let path = format!("/v2/search/{}", self.namespace);
        let resp = self.req_post(&path).json(&body).send().await?;
        parse_response(resp).await
    }

    /// `POST /v2/search/:ns` with `hydrate: true` — fetch one page of matching
    /// objects with their full data.
    pub async fn search_full<T: DeserializeOwned>(
        &self,
        filters: &[SearchFilter],
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ListPageFull<T>> {
        let body = SearchBody { filters, limit, cursor, hydrate: true };
        let path = format!("/v2/search/{}", self.namespace);
        let resp = self.req_post(&path).json(&body).send().await?;
        parse_response(resp).await
    }

    /// Collect **all** keys matching `filters` across every page (non-hydrated).
    ///
    /// Pages are fetched sequentially. `page_size` sets the per-page `limit`.
    pub async fn search_all(
        &self,
        filters: &[SearchFilter],
        page_size: Option<u32>,
    ) -> Result<Vec<String>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let page = self.search(filters, page_size, cursor.as_deref()).await?;
            all.extend(page.keys);
            match page.cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        Ok(all)
    }

    /// Collect **all** matching hydrated objects across every page.
    pub async fn search_all_full<T: DeserializeOwned>(
        &self,
        filters: &[SearchFilter],
        page_size: Option<u32>,
    ) -> Result<Vec<HydratedItem<T>>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let page = self.search_full::<T>(filters, page_size, cursor.as_deref()).await?;
            all.extend(page.items);
            match page.cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        Ok(all)
    }

    // -----------------------------------------------------------------------
    // Bulk operations
    // -----------------------------------------------------------------------

    /// `POST /v2/bulk/get` — fetch multiple objects in a single request.
    ///
    /// Items are processed through the L1 → L2 → KV → Cold waterfall per key
    /// and returned in the same order as `keys`. Missing keys have `ok: false`
    /// in the response — this is not a top-level error.
    ///
    /// # Constraints
    ///
    /// - All keys must pass the nanoid constraint (`A-Za-z0-9_-`, 5–21 chars).
    ///   One invalid key rejects the entire request with `InvalidKey`.
    /// - Maximum keys per request: deployment-specific (default 100).
    ///   Exceeding this returns `BulkTooLarge`.
    pub async fn bulk_get<T: DeserializeOwned>(
        &self,
        keys: &[String],
    ) -> Result<BulkGetResponse<T>> {
        let body = BulkGetBody { ns: &self.namespace, keys };
        let resp = self.req_post("/v2/bulk/get").json(&body).send().await?;
        parse_response(resp).await
    }

    /// `POST /v2/bulk/create` — create multiple objects with server-generated keys.
    ///
    /// Each item gets a nanoid(21) key. Items are processed concurrently on the
    /// server. The response is in the same order as `items`.
    ///
    /// Slightly cheaper per item than `bulk_set` because no L2 invalidation DEL
    /// is needed for brand-new keys.
    ///
    /// Total cost for all items is charged **before** any writes; partial
    /// failures still consume quota.
    pub async fn bulk_create<T: Serialize>(
        &self,
        items: &[BulkCreateItem<T>],
    ) -> Result<BulkCreateResult> {
        let body = BulkCreateBody { ns: &self.namespace, items };
        let resp = self.req_post("/v2/bulk/create").json(&body).send().await?;
        parse_response(resp).await
    }

    /// `POST /v2/bulk/set` — upsert multiple objects at caller-supplied keys.
    ///
    /// Existing keys are fully replaced; missing keys are created. Items are
    /// processed concurrently on the server. The response is in the same order
    /// as `items`.
    ///
    /// Total cost for all items is charged **before** any writes.
    pub async fn bulk_set<T: Serialize>(
        &self,
        items: &[BulkSetItem<T>],
    ) -> Result<BulkWriteResponse> {
        let body = BulkSetBody { ns: &self.namespace, items };
        let resp = self.req_post("/v2/bulk/set").json(&body).send().await?;
        parse_response(resp).await
    }

    /// `POST /v2/bulk/delete` — delete multiple objects in a single request.
    ///
    /// Performs a parallel delete across all tiers for each key. Non-existent
    /// keys are silently skipped (idempotent). The response is in the same
    /// order as `keys`.
    pub async fn bulk_delete(&self, keys: &[String]) -> Result<BulkWriteResponse> {
        let body = BulkDeleteBody { ns: &self.namespace, keys };
        let resp = self.req_post("/v2/bulk/delete").json(&body).send().await?;
        parse_response(resp).await
    }
}
