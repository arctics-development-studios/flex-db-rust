use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

// ---------------------------------------------------------------------------
// Read responses
// ---------------------------------------------------------------------------

/// Response from `GET /v2/o/:ns/:key`.
///
/// `data` is the JSON value that was stored with [`Namespace::set`](crate::Namespace::set)
/// or [`Namespace::create`](crate::Namespace::create).
///
/// Note: `sp` (search properties) are write-only — they are not returned in
/// get responses. Use [`Namespace::search`](crate::Namespace::search) to query by them.
#[derive(Debug, Clone, Deserialize)]
pub struct GetResponse<T> {
    pub data: T,
}

/// One item in a hydrated list or search result (`hydrate=true`).
///
/// `data` is `None` when the object could not be fetched from any storage tier
/// (e.g. it was deleted between the list scan and the fetch).
#[derive(Debug, Clone, Deserialize)]
pub struct HydratedItem<T> {
    pub key: String,
    pub data: Option<T>,
}

/// A single page of object keys returned by list or search (non-hydrated).
#[derive(Debug, Clone, Deserialize)]
pub struct ListPage {
    pub keys: Vec<String>,
    /// Opaque pagination token. `Some` when more pages exist, `None` on the
    /// last page. Pass verbatim to the next call's `cursor` parameter.
    pub cursor: Option<String>,
}

/// A single page of hydrated objects returned by list or search (`hydrate=true`).
#[derive(Debug, Clone, Deserialize)]
pub struct ListPageFull<T> {
    pub items: Vec<HydratedItem<T>>,
    /// Opaque pagination token. `Some` when more pages exist, `None` on the
    /// last page. Pass verbatim to the next call's `cursor` parameter.
    pub cursor: Option<String>,
}

/// Response from `GET /health`.
#[derive(Debug, Clone, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

// ---------------------------------------------------------------------------
// Bulk response types
// ---------------------------------------------------------------------------

/// One item in a `bulk_get` response.
///
/// `ok: true` means the object was found and `data` is populated.
/// `ok: false` means the key does not exist at any storage tier.
#[derive(Debug, Clone, Deserialize)]
pub struct BulkGetItem<T> {
    pub key: String,
    pub ok: bool,
    pub data: Option<T>,
}

/// Response from [`Namespace::bulk_get`](crate::Namespace::bulk_get).
///
/// Items are returned in the same order as the input `keys` slice.
#[derive(Debug, Clone, Deserialize)]
pub struct BulkGetResponse<T> {
    pub items: Vec<BulkGetItem<T>>,
}

/// One item in a `bulk_set` or `bulk_delete` response.
#[derive(Debug, Clone, Deserialize)]
pub struct BulkWriteItem {
    pub ok: bool,
    pub error: Option<String>,
}

/// Response from [`Namespace::bulk_set`](crate::Namespace::bulk_set) or
/// [`Namespace::bulk_delete`](crate::Namespace::bulk_delete).
///
/// Items are returned in the same order as the input slice.
#[derive(Debug, Clone, Deserialize)]
pub struct BulkWriteResponse {
    pub items: Vec<BulkWriteItem>,
}

/// One item in a `bulk_create` response.
///
/// On success, `id` holds the auto-generated nanoid(21) key.
#[derive(Debug, Clone, Deserialize)]
pub struct BulkCreateResultItem {
    pub ok: bool,
    pub id: Option<String>,
    pub error: Option<String>,
}

/// Response from [`Namespace::bulk_create`](crate::Namespace::bulk_create).
///
/// Items are returned in the same order as the input slice.
#[derive(Debug, Clone, Deserialize)]
pub struct BulkCreateResult {
    pub items: Vec<BulkCreateResultItem>,
}

// ---------------------------------------------------------------------------
// Bulk write input types
// ---------------------------------------------------------------------------

/// One item for [`Namespace::bulk_create`](crate::Namespace::bulk_create).
///
/// `sp` is an optional flat map of searchable properties. See the
/// [search properties](https://flex.arctics.dev/docs/search) guide for key/value rules.
#[derive(Debug, Clone, Serialize)]
pub struct BulkCreateItem<T: Serialize> {
    pub data: T,
    /// Optional search properties. Keys: `A-Za-z0-9_`, max 64 chars.
    /// Values: any JSON scalar (`string`, `number`, `boolean`, `null`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp: Option<Map<String, Value>>,
}

/// One item for [`Namespace::bulk_set`](crate::Namespace::bulk_set).
///
/// `key` must pass the nanoid constraint: alphabet `A-Za-z0-9_-`, length 5–21.
#[derive(Debug, Clone, Serialize)]
pub struct BulkSetItem<T: Serialize> {
    pub key: String,
    pub data: T,
    /// Optional search properties. See [`BulkCreateItem::sp`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp: Option<Map<String, Value>>,
}

// ---------------------------------------------------------------------------
// Internal request body types (not pub)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub(crate) struct WriteBody<'a, T: Serialize> {
    pub data: &'a T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp: Option<&'a Map<String, Value>>,
}

#[derive(Serialize)]
pub(crate) struct BulkGetBody<'a> {
    pub ns: &'a str,
    pub keys: &'a [String],
}

#[derive(Serialize)]
pub(crate) struct BulkCreateBody<'a, T: Serialize> {
    pub ns: &'a str,
    pub items: &'a [BulkCreateItem<T>],
}

#[derive(Serialize)]
pub(crate) struct BulkSetBody<'a, T: Serialize> {
    pub ns: &'a str,
    pub items: &'a [BulkSetItem<T>],
}

#[derive(Serialize)]
pub(crate) struct BulkDeleteBody<'a> {
    pub ns: &'a str,
    pub keys: &'a [String],
}

#[derive(Serialize)]
pub(crate) struct SearchBody<'a> {
    pub filters: &'a [crate::filter::SearchFilter],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<&'a str>,
    pub hydrate: bool,
}
