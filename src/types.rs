use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

/// Metadata stored alongside every object, returned on reads.
#[derive(Debug, Clone, Deserialize)]
pub struct ObjectMeta {
    /// Storage tier: `true` = warm (DynamoDB), `false` = cold (S3).
    pub w: bool,
    /// Serialized byte size of `data` at last write.
    pub s: u64,
    /// Unix timestamp (seconds) of the last write.
    pub lut: u64,
    /// Reserved field; always `0`.
    pub upi: u64,
    /// Search parameters stored with this object (`metadata.sp`).
    pub sp: Map<String, Value>,
}

/// Metadata to include when writing an object.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WriteMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp: Option<Map<String, Value>>,
}

// ---------------------------------------------------------------------------
// Read responses
// ---------------------------------------------------------------------------

/// Response from `GET /v1/:key`.
#[derive(Debug, Clone, Deserialize)]
pub struct GetResponse<T> {
    pub key: String,
    pub data: T,
    pub metadata: ObjectMeta,
}

/// One item in a hydrated list or search result (`full=true`).
/// `data` is `None` when the object could not be fetched from storage.
#[derive(Debug, Clone, Deserialize)]
pub struct HydratedItem<T> {
    pub key: String,
    pub data: Option<T>,
}

/// A single page of object keys (non-hydrated).
#[derive(Debug, Clone, Deserialize)]
pub struct ListPage {
    pub keys: Vec<String>,
    /// Present only when more pages exist. Absent on the last page.
    pub cursor: Option<String>,
}

/// A single page of hydrated objects (`full=true`).
#[derive(Debug, Clone, Deserialize)]
pub struct ListPageFull<T> {
    pub keys: Vec<HydratedItem<T>>,
    /// Present only when more pages exist. Absent on the last page.
    pub cursor: Option<String>,
}

/// Response from `POST /v1/update` (filter-based partial update).
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateFilterResponse {
    /// Number of objects successfully patched in this call.
    pub updated: u64,
    /// Present when more matching objects exist beyond this page.
    pub cursor: Option<String>,
}

/// Response from `GET /health`.
#[derive(Debug, Clone, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

// ---------------------------------------------------------------------------
// Bulk write item types
// ---------------------------------------------------------------------------

/// One item for `bulk_create`.
#[derive(Debug, Clone, Serialize)]
pub struct BulkCreateItem<T: Serialize> {
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<WriteMetadata>,
}

/// One item for `bulk_set`.
#[derive(Debug, Clone, Serialize)]
pub struct BulkSetItem<T: Serialize> {
    pub key: String,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<WriteMetadata>,
}

// ---------------------------------------------------------------------------
// Internal request body types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub(crate) struct CreateBody<'a, T: Serialize> {
    pub data: &'a T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<WriteMetadata>,
}

#[derive(Serialize)]
pub(crate) struct BulkCreateBody<'a, T: Serialize> {
    pub items: &'a [BulkCreateItem<T>],
}

#[derive(Serialize)]
pub(crate) struct BulkSetBody<'a, T: Serialize> {
    pub items: &'a [BulkSetItem<T>],
}

#[derive(Serialize)]
pub(crate) struct BulkDeleteBody<'a> {
    pub keys: &'a [String],
}

#[derive(Serialize)]
pub(crate) struct SearchBody<'a> {
    pub filters: &'a [crate::filter::SearchFilter],
    pub options: SearchOptions<'a>,
}

#[derive(Serialize)]
pub(crate) struct SearchOptions<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<&'a str>,
    pub full: bool,
}

#[derive(Serialize)]
pub(crate) struct UpdateWhereBody<'a, T: Serialize> {
    pub filters: &'a [crate::filter::SearchFilter],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<&'a T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<WriteMetadata>,
    pub options: UpdateWhereOptions<'a>,
}

#[derive(Serialize)]
pub(crate) struct UpdateWhereOptions<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<&'a str>,
}
