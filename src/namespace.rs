use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

use crate::client::{parse_response, FlexDbInner};
use crate::error::Result;
use crate::filter::SearchFilter;
use crate::types::{
    BulkCreateBody, BulkCreateItem, BulkDeleteBody, BulkSetBody, BulkSetItem, CreateBody,
    GetResponse, HydratedItem, ListPage, ListPageFull, SearchBody, SearchOptions,
    UpdateFilterResponse, UpdateOneBody, UpdateWhereBody, UpdateWhereOptions, WriteMetadata,
};

// ---------------------------------------------------------------------------
// Namespace client
// ---------------------------------------------------------------------------

/// A data client scoped to a specific namespace within a Flex DB database.
///
/// Obtain via [`FlexDb::namespace`](crate::FlexDb::namespace).
///
/// `Namespace` is `Clone + Send + Sync` — the underlying HTTP client and
/// credentials are shared through an `Arc`. Spawn it across tasks freely.
///
/// # Parallel requests
///
/// All methods take `&self`, so you can issue concurrent requests with
/// `tokio::join!` or `futures::join_all`:
///
/// ```rust,no_run
/// # async fn example(ns: flex_db::Namespace) -> Result<(), flex_db::Error> {
/// let (a, b) = tokio::join!(
///     ns.get::<serde_json::Value>("key1"),
///     ns.get::<serde_json::Value>("key2"),
/// );
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
pub struct Namespace {
    pub(crate) inner: Arc<FlexDbInner>,
    pub(crate) namespace: String,
}

impl Namespace {
    // -----------------------------------------------------------------------
    // Helper: build a request with both required headers
    // -----------------------------------------------------------------------

    fn req_get(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.inner.base_url, path);
        self.inner
            .auth(self.inner.http.get(&url))
            .header("X-Namespace", &self.namespace)
    }

    fn req_post(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.inner.base_url, path);
        self.inner
            .auth(self.inner.http.post(&url))
            .header("X-Namespace", &self.namespace)
    }

    fn req_put(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.inner.base_url, path);
        self.inner
            .auth(self.inner.http.put(&url))
            .header("X-Namespace", &self.namespace)
    }

    fn req_delete(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.inner.base_url, path);
        self.inner
            .auth(self.inner.http.delete(&url))
            .header("X-Namespace", &self.namespace)
    }

    // -----------------------------------------------------------------------
    // Single-object operations
    // -----------------------------------------------------------------------

    /// `POST /v1` — create an object with an auto-generated key.
    ///
    /// Returns the new key (21-char nanoid).
    ///
    /// `sp` is the optional map of search parameters stored with the object.
    pub async fn create<T: Serialize>(
        &self,
        data: &T,
        sp: Option<&Map<String, Value>>,
    ) -> Result<String> {
        let metadata = sp.map(|m| WriteMetadata { sp: Some(m.clone()) });
        let body = CreateBody { data, metadata };
        let resp = self.req_post("/v1").json(&body).send().await?;
        let val: serde_json::Value = parse_response(resp).await?;
        Ok(val["key"].as_str().unwrap_or_default().to_owned())
    }

    /// `GET /v1/:key` — read a single object and its metadata.
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<GetResponse<T>> {
        let resp = self.req_get(&format!("/v1/{key}")).send().await?;
        parse_response(resp).await
    }

    /// `PUT /v1/:key` — fully replace (upsert) an object. Returns the key.
    ///
    /// If the key does not exist it is created. Tier assignment is
    /// recalculated on every write based on the new payload size.
    pub async fn set<T: Serialize>(
        &self,
        key: &str,
        data: &T,
        sp: Option<&Map<String, Value>>,
    ) -> Result<String> {
        let metadata = sp.map(|m| WriteMetadata { sp: Some(m.clone()) });
        let body = CreateBody { data, metadata };
        let resp = self.req_put(&format!("/v1/{key}")).json(&body).send().await?;
        let val: serde_json::Value = parse_response(resp).await?;
        Ok(val["key"].as_str().unwrap_or_default().to_owned())
    }

    /// `DELETE /v1/:key` — delete an object from all storage tiers.
    ///
    /// Always succeeds (returns `Ok(())`) even when the key does not exist.
    pub async fn delete(&self, key: &str) -> Result<()> {
        let resp = self.req_delete(&format!("/v1/{key}")).send().await?;
        parse_response::<serde_json::Value>(resp).await?;
        Ok(())
    }

    /// `POST /v1/updateOne/:key` — shallow merge patch on a single object.
    ///
    /// The object must already exist; `ERR_NOT_FOUND` is returned otherwise.
    ///
    /// - Pass `data: None` to leave the stored `data` unchanged.
    /// - Pass `sp: None` to leave the stored `metadata.sp` unchanged.
    /// - If both existing and patch `data` are JSON objects, keys are merged
    ///   shallowly. Otherwise the existing value is replaced entirely.
    pub async fn update_one<T: Serialize>(
        &self,
        key: &str,
        data: Option<&T>,
        sp: Option<&Map<String, Value>>,
    ) -> Result<String> {
        let metadata = sp.map(|m| WriteMetadata { sp: Some(m.clone()) });
        let body = UpdateOneBody { data, metadata };
        let resp = self
            .req_post(&format!("/v1/updateOne/{key}"))
            .json(&body)
            .send()
            .await?;
        let val: serde_json::Value = parse_response(resp).await?;
        Ok(val["key"].as_str().unwrap_or_default().to_owned())
    }

    // -----------------------------------------------------------------------
    // List operations
    // -----------------------------------------------------------------------

    /// `GET /v1/list` — fetch one page of object keys (non-hydrated).
    pub async fn list(&self, limit: Option<u32>, cursor: Option<&str>) -> Result<ListPage> {
        let mut req = self.req_get("/v1/list");
        if let Some(l) = limit {
            req = req.query(&[("limit", l.to_string())]);
        }
        if let Some(c) = cursor {
            req = req.query(&[("cursor", c)]);
        }
        let resp = req.send().await?;
        parse_response(resp).await
    }

    /// `GET /v1/list?full=true` — fetch one page of hydrated objects.
    pub async fn list_full<T: DeserializeOwned>(
        &self,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ListPageFull<T>> {
        let mut req = self.req_get("/v1/list").query(&[("full", "true")]);
        if let Some(l) = limit {
            req = req.query(&[("limit", l.to_string())]);
        }
        if let Some(c) = cursor {
            req = req.query(&[("cursor", c)]);
        }
        let resp = req.send().await?;
        parse_response(resp).await
    }

    /// Collect **all** object keys across every page (non-hydrated).
    ///
    /// Pages are fetched sequentially because each cursor depends on the
    /// previous page. `page_size` defaults to the server maximum (100).
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

    /// Collect **all** hydrated objects across every page.
    pub async fn list_all_full<T: DeserializeOwned>(
        &self,
        page_size: Option<u32>,
    ) -> Result<Vec<HydratedItem<T>>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let page = self.list_full::<T>(page_size, cursor.as_deref()).await?;
            all.extend(page.keys);
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

    /// `POST /v1/search` — fetch one page of matching keys (non-hydrated).
    ///
    /// `filters` must be non-empty; passing an empty slice returns
    /// `Error::Api { code: ApiErrorCode::MissingFilter, .. }`.
    pub async fn search(
        &self,
        filters: &[SearchFilter],
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ListPage> {
        let body = SearchBody {
            filters,
            options: SearchOptions { limit, cursor, full: false },
        };
        let resp = self.req_post("/v1/search").json(&body).send().await?;
        parse_response(resp).await
    }

    /// `POST /v1/search` with `full: true` — fetch one page of hydrated objects.
    pub async fn search_full<T: DeserializeOwned>(
        &self,
        filters: &[SearchFilter],
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ListPageFull<T>> {
        let body = SearchBody {
            filters,
            options: SearchOptions { limit, cursor, full: true },
        };
        let resp = self.req_post("/v1/search").json(&body).send().await?;
        parse_response(resp).await
    }

    /// Collect **all** matching keys across every page (non-hydrated).
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
            let page = self
                .search_full::<T>(filters, page_size, cursor.as_deref())
                .await?;
            all.extend(page.keys);
            match page.cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        Ok(all)
    }

    // -----------------------------------------------------------------------
    // Filter-based partial update
    // -----------------------------------------------------------------------

    /// `POST /v1/update` — shallow merge patch on all objects matching `filters`.
    ///
    /// Processes one page of matches per call. Use `cursor` from the response
    /// to fetch subsequent pages, or call [`update_all_where`](Self::update_all_where)
    /// to process all pages automatically.
    pub async fn update_where<T: Serialize>(
        &self,
        filters: &[SearchFilter],
        data: Option<&T>,
        sp: Option<&Map<String, Value>>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<UpdateFilterResponse> {
        let metadata = sp.map(|m| WriteMetadata { sp: Some(m.clone()) });
        let body = UpdateWhereBody {
            filters,
            data,
            metadata,
            options: UpdateWhereOptions { limit, cursor },
        };
        let resp = self.req_post("/v1/update").json(&body).send().await?;
        parse_response(resp).await
    }

    /// `POST /v1/update` — apply merge patch to **all** matching objects.
    ///
    /// Follows cursors automatically. Returns the total number of objects patched.
    pub async fn update_all_where<T: Serialize>(
        &self,
        filters: &[SearchFilter],
        data: Option<&T>,
        sp: Option<&Map<String, Value>>,
        page_size: Option<u32>,
    ) -> Result<u64> {
        let mut total = 0u64;
        let mut cursor: Option<String> = None;
        loop {
            let resp = self
                .update_where(filters, data, sp, page_size, cursor.as_deref())
                .await?;
            total += resp.updated;
            match resp.cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        Ok(total)
    }

    // -----------------------------------------------------------------------
    // Bulk operations
    // -----------------------------------------------------------------------

    /// `POST /v1/bulk/create` — create up to 50 objects in parallel on the server.
    ///
    /// Returns auto-generated keys in the same order as `items`.
    pub async fn bulk_create<T: Serialize>(
        &self,
        items: &[BulkCreateItem<T>],
    ) -> Result<Vec<String>> {
        let body = BulkCreateBody { items };
        let resp = self.req_post("/v1/bulk/create").json(&body).send().await?;
        let val: serde_json::Value = parse_response(resp).await?;
        let keys = val["keys"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        Ok(keys)
    }

    /// `POST /v1/bulk/set` — upsert up to 50 objects in parallel on the server.
    ///
    /// Each item must include an explicit key. Existing keys are fully replaced;
    /// missing keys are created. Returns keys in the same order as `items`.
    pub async fn bulk_set<T: Serialize>(&self, items: &[BulkSetItem<T>]) -> Result<Vec<String>> {
        let body = BulkSetBody { items };
        let resp = self.req_post("/v1/bulk/set").json(&body).send().await?;
        let val: serde_json::Value = parse_response(resp).await?;
        let keys = val["keys"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        Ok(keys)
    }

    /// `DELETE /v1/bulk/delete` — delete up to 50 objects in parallel on the server.
    ///
    /// Non-existent keys are silently skipped. Always returns `Ok(())`.
    pub async fn bulk_delete(&self, keys: &[String]) -> Result<()> {
        let body = BulkDeleteBody { keys };
        let resp = self.req_delete("/v1/bulk/delete").json(&body).send().await?;
        parse_response::<serde_json::Value>(resp).await?;
        Ok(())
    }
}
