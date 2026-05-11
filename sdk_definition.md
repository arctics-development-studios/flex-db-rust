# Flex DB Rust SDK — API Reference

**Crate:** `flex-db`  
**Version:** 2.3.1  
**Edition:** Rust 2024  
**API compatibility:** Flex DB API v2.3.1

---

## Table of Contents

1. [Installation](#installation)
2. [Entry Points](#entry-points)
3. [FlexDb — Top-Level Client](#flexdb--top-level-client)
4. [Namespace — Data Client](#namespace--data-client)
   - [Single-Object Operations](#single-object-operations)
   - [List Operations](#list-operations)
   - [Search Operations](#search-operations)
   - [Filter-Based Update](#filter-based-update)
   - [Bulk Operations](#bulk-operations)
5. [Types](#types)
   - [Request Types](#request-types)
   - [Response Types](#response-types)
6. [SearchFilter & FilterOp](#searchfilter--filterop)
7. [Error Handling](#error-handling)
8. [Parallelism](#parallelism)
9. [Pagination](#pagination)
10. [Merge Semantics](#merge-semantics)

---

## Installation

```toml
[dependencies]
flex-db = "2.3.1"
tokio   = { version = "1", features = ["rt-multi-thread", "macros"] }
serde   = { version = "1", features = ["derive"] }
```

---

## Entry Points

```rust
use flex_db::{
    FlexDb, Namespace,
    SearchFilter, FilterOp,
    BulkCreateItem, BulkSetItem,
    GetResponse, HydratedItem, ListPage, ListPageFull,
    ObjectMeta, UpdateFilterResponse, WriteMetadata,
    Error, ApiErrorCode, Result,
};
```

---

## FlexDb — Top-Level Client

```rust
pub struct FlexDb { /* Arc-wrapped internals */ }
```

`FlexDb` is `Clone + Send + Sync`. Cloning is cheap — all clones share the same connection pool and credentials.

### Constructors

```rust
pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self
```

- `base_url`: scheme + host, no trailing slash. Example: `"https://api.flexdb.io"`.
- `token`: raw JWT string (without the `Bearer ` prefix).

### Methods

```rust
pub fn namespace(&self, namespace: impl Into<String>) -> Namespace
```

Returns a [`Namespace`](#namespace--data-client) scoped to the given namespace string. Zero-cost except for the string allocation.

```rust
pub async fn health(&self) -> Result<HealthResponse>
```

`GET /health` — no authentication required. Returns `{ status: "healthy" }` when the server is up.

---

## Namespace — Data Client

```rust
pub struct Namespace { /* Arc-wrapped, namespace: String */ }
```

`Namespace` is `Clone + Send + Sync`. Obtain via `FlexDb::namespace()`.

All methods attach `Authorization: Bearer <token>` and `X-Namespace: <namespace>` to every request.

---

### Single-Object Operations

#### create

```rust
pub async fn create<T: Serialize>(
    &self,
    data: &T,
    sp: Option<&Map<String, Value>>,
) -> Result<String>
```

`POST /v1` — creates an object with an auto-generated 21-char nanoid key.

- `data`: any `Serialize` type. Can be a struct, `serde_json::Value`, string, number, etc.
- `sp`: optional search parameters stored alongside the object for later filtering.
- Returns the new key string on success.

---

#### get

```rust
pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<GetResponse<T>>
```

`GET /v1/:key` — retrieves a single object and its metadata.

- Returns `Error::Api { code: ApiErrorCode::NotFound, .. }` if the key does not exist.

---

#### set

```rust
pub async fn set<T: Serialize>(
    &self,
    key: &str,
    data: &T,
    sp: Option<&Map<String, Value>>,
) -> Result<String>
```

`PUT /v1/:key` — fully replaces (upserts) an object. If the key does not exist it is created. If it exists, both `data` and `sp` are completely replaced.

Returns the key string.

---

#### delete

```rust
pub async fn delete(&self, key: &str) -> Result<()>
```

`DELETE /v1/:key` — deletes an object from all storage tiers. Always returns `Ok(())`, even when the key does not exist.

---

#### update_one

```rust
pub async fn update_one<T: Serialize>(
    &self,
    key: &str,
    data: Option<&T>,
    sp: Option<&Map<String, Value>>,
) -> Result<String>
```

`POST /v1/updateOne/:key` — shallow merge patch on a single object. The object must already exist; returns `ApiErrorCode::NotFound` otherwise.

- `data: None` — field is **omitted from the request** (not sent as `null`), so the server preserves the existing value.
- `sp: None` — field is **omitted from the request**, so the server preserves the existing `metadata.sp`.
- If both existing and patch `data` are JSON objects, keys are merged shallowly. Otherwise the existing value is replaced entirely. See [Merge Semantics](#merge-semantics).

Returns the key string.

---

### List Operations

#### list

```rust
pub async fn list(
    &self,
    limit: Option<u32>,
    cursor: Option<&str>,
) -> Result<ListPage>
```

`GET /v1/list` — returns one page of object keys (non-hydrated). `limit` defaults to 20, capped at 100 server-side.

#### list_full

```rust
pub async fn list_full<T: DeserializeOwned>(
    &self,
    limit: Option<u32>,
    cursor: Option<&str>,
) -> Result<ListPageFull<T>>
```

`GET /v1/list?full=true` — returns one page of hydrated objects. Each item includes the stored `data` value. `data` is `None` for items that could not be fetched.

#### list_all

```rust
pub async fn list_all(&self, page_size: Option<u32>) -> Result<Vec<String>>
```

Collects all object keys across every page by following cursors sequentially. `page_size` is the `limit` per request; `None` uses the server default.

#### list_all_full

```rust
pub async fn list_all_full<T: DeserializeOwned>(
    &self,
    page_size: Option<u32>,
) -> Result<Vec<HydratedItem<T>>>
```

Collects all hydrated objects across every page.

---

### Search Operations

All search methods require at least one filter. Passing an empty `filters` slice results in `ApiErrorCode::MissingFilter`. All filters are AND-ed.

#### search

```rust
pub async fn search(
    &self,
    filters: &[SearchFilter],
    limit: Option<u32>,
    cursor: Option<&str>,
) -> Result<ListPage>
```

`POST /v1/search` — returns one page of matching keys (non-hydrated).

#### search_full

```rust
pub async fn search_full<T: DeserializeOwned>(
    &self,
    filters: &[SearchFilter],
    limit: Option<u32>,
    cursor: Option<&str>,
) -> Result<ListPageFull<T>>
```

`POST /v1/search` with `full: true` — returns one page of hydrated matching objects.

#### search_all

```rust
pub async fn search_all(
    &self,
    filters: &[SearchFilter],
    page_size: Option<u32>,
) -> Result<Vec<String>>
```

Collects all matching keys across every page.

#### search_all_full

```rust
pub async fn search_all_full<T: DeserializeOwned>(
    &self,
    filters: &[SearchFilter],
    page_size: Option<u32>,
) -> Result<Vec<HydratedItem<T>>>
```

Collects all matching hydrated objects across every page.

---

### Filter-Based Update

#### update_where

```rust
pub async fn update_where<T: Serialize>(
    &self,
    filters: &[SearchFilter],
    data: Option<&T>,
    sp: Option<&Map<String, Value>>,
    limit: Option<u32>,
    cursor: Option<&str>,
) -> Result<UpdateFilterResponse>
```

`POST /v1/update` — finds objects matching `filters` and applies a shallow merge patch. Processes one page per call. Use `UpdateFilterResponse::cursor` to continue to the next page.

#### update_all_where

```rust
pub async fn update_all_where<T: Serialize>(
    &self,
    filters: &[SearchFilter],
    data: Option<&T>,
    sp: Option<&Map<String, Value>>,
    page_size: Option<u32>,
) -> Result<u64>
```

Applies the merge patch to **all** matching objects across every page. Returns the total count of objects successfully patched.

---

### Bulk Operations

All bulk operations process items concurrently on the server. The default server limit is 50 items per request; exceeding it returns `ApiErrorCode::BulkTooLarge`.

#### bulk_create

```rust
pub async fn bulk_create<T: Serialize>(
    &self,
    items: &[BulkCreateItem<T>],
) -> Result<Vec<String>>
```

`POST /v1/bulk/create` — creates up to 50 objects in one request. Returns auto-generated keys in the same order as `items`.

#### bulk_set

```rust
pub async fn bulk_set<T: Serialize>(
    &self,
    items: &[BulkSetItem<T>],
) -> Result<Vec<String>>
```

`POST /v1/bulk/set` — upserts up to 50 objects. Each item must include an explicit key. Existing keys are fully replaced; missing keys are created. Returns keys echoed in input order.

#### bulk_delete

```rust
pub async fn bulk_delete(&self, keys: &[String]) -> Result<()>
```

`DELETE /v1/bulk/delete` — deletes up to 50 objects. Non-existent keys are silently skipped. Always returns `Ok(())`.

---

## Types

### Request Types

#### WriteMetadata

```rust
pub struct WriteMetadata {
    pub sp: Option<Map<String, Value>>,
}
```

Optional metadata to include in write requests. Omitting `sp` (or passing `None`) defaults to an empty map on the server.

#### BulkCreateItem\<T\>

```rust
pub struct BulkCreateItem<T: Serialize> {
    pub data: T,
    pub metadata: Option<WriteMetadata>,
}
```

One item for `bulk_create`.

#### BulkSetItem\<T\>

```rust
pub struct BulkSetItem<T: Serialize> {
    pub key: String,
    pub data: T,
    pub metadata: Option<WriteMetadata>,
}
```

One item for `bulk_set`. `key` must be an existing or new key in the namespace.

---

### Response Types

#### HealthResponse

```rust
pub struct HealthResponse {
    pub status: String,   // "healthy"
}
```

#### GetResponse\<T\>

```rust
pub struct GetResponse<T> {
    pub key: String,
    pub data: T,
    pub metadata: ObjectMeta,
}
```

#### ObjectMeta

```rust
pub struct ObjectMeta {
    pub w: bool,                      // true = warm (DynamoDB), false = cold (S3)
    pub s: u64,                       // byte size of data at last write
    pub lut: u64,                     // Unix timestamp of last write (seconds)
    pub upi: u64,                     // reserved, always 0
    pub sp: Map<String, Value>,       // stored search parameters
}
```

#### ListPage

```rust
pub struct ListPage {
    pub keys: Vec<String>,
    pub cursor: Option<String>,       // absent on last page
}
```

#### ListPageFull\<T\>

```rust
pub struct ListPageFull<T> {
    pub keys: Vec<HydratedItem<T>>,
    pub cursor: Option<String>,
}
```

#### HydratedItem\<T\>

```rust
pub struct HydratedItem<T> {
    pub key: String,
    pub data: Option<T>,              // None if object could not be fetched
}
```

#### UpdateFilterResponse

```rust
pub struct UpdateFilterResponse {
    pub updated: u64,                 // objects patched in this page
    pub cursor: Option<String>,       // absent on last page
}
```

---

## SearchFilter & FilterOp

### FilterOp

```rust
pub enum FilterOp {
    Eq,   // ==
    Neq,  // !=
    Gt,   // >
    Gte,  // >=
    Lt,   // <
    Lte,  // <=
    Sw,   // starts_with (begins_with in DynamoDB)
    Ex,   // attribute_exists
}
```

### SearchFilter

```rust
pub struct SearchFilter {
    pub field: String,
    pub op: FilterOp,
    pub value: serde_json::Value,
}
```

All filters in a request are AND-ed. OR is not supported. `field` must be a top-level key of `metadata.sp`.

#### Convenience constructors

| Constructor | Equivalent |
|---|---|
| `SearchFilter::eq(field, value)` | `field == value` |
| `SearchFilter::neq(field, value)` | `field != value` |
| `SearchFilter::gt(field, value)` | `field > value` |
| `SearchFilter::gte(field, value)` | `field >= value` |
| `SearchFilter::lt(field, value)` | `field < value` |
| `SearchFilter::lte(field, value)` | `field <= value` |
| `SearchFilter::starts_with(field, prefix)` | `field` starts with `prefix` |
| `SearchFilter::exists(field)` | `field` is present in `sp` |
| `SearchFilter::new(field, op, value)` | general form |

`value` accepts any `Serialize` type in the convenience constructors. The value is converted to `serde_json::Value` at construction time.

---

## Error Handling

```rust
pub type Result<T> = std::result::Result<T, Error>;

pub enum Error {
    Api { code: ApiErrorCode, message: String },
    Transport(reqwest::Error),
    Deserialize(serde_json::Error),
}
```

Always match on `ApiErrorCode` variants, not on `message` strings — messages may change between server versions.

```rust
use flex_db::{ApiErrorCode, Error};

match ns.get::<MyType>("key").await {
    Ok(obj) => { /* use obj */ }
    Err(Error::Api { code: ApiErrorCode::NotFound, .. }) => { /* handle 404 */ }
    Err(Error::Api { code: ApiErrorCode::RateLimitSecond, .. }) => { /* back off */ }
    Err(e) => eprintln!("unexpected: {e}"),
}
```

### ApiErrorCode variants

| Variant | API code | HTTP |
|---|---|---|
| `MissingAuth` | `ERR_MISSING_AUTH` | 401 |
| `Unauthorized` | `ERR_UNAUTHORIZED` | 401 |
| `MissingNamespace` | `ERR_MISSING_NAMESPACE` | 400 |
| `PermissionDenied` | `ERR_PERMISSION_DENIED` | 403 |
| `Forbidden` | `ERR_FORBIDDEN` | 403 |
| `NotFound` | `ERR_NOT_FOUND` | 404 |
| `MissingFilter` | `ERR_MISSING_FILTER` | 400 |
| `RateLimitSecond` | `ERR_RATE_LIMIT_SECOND` | 429 |
| `RateLimitMonth` | `ERR_RATE_LIMIT_MONTH` | 429 |
| `RequestTooLarge` | `ERR_REQUEST_TOO_LARGE` | 413 |
| `BulkTooLarge` | `ERR_BULK_TOO_LARGE` | 413 |
| `InvalidRequest` | `ERR_INVALID_REQUEST` | 400 |
| `StoreFailed` | `ERR_STORE_FAILED` | 500 |
| `DeleteFailed` | `ERR_DELETE_FAILED` | 500 |
| `Internal` | `ERR_INTERNAL` | 500 |
| `Unknown(String)` | any unrecognized code | — |

---

## Parallelism

All methods take `&self`. `FlexDb` and `Namespace` are `Clone + Send + Sync`. You can fire multiple requests concurrently with `tokio::join!` or `tokio::spawn`:

```rust
// Concurrent reads
let (a, b, c) = tokio::join!(
    ns.get::<T>("key1"),
    ns.get::<T>("key2"),
    ns.get::<T>("key3"),
);

// Concurrent across namespaces
let ns1 = client.namespace("users");
let ns2 = client.namespace("sessions");
let (u, s) = tokio::join!(
    ns1.list(None, None),
    ns2.list(None, None),
);
```

Cloning `Namespace` (or `FlexDb`) is safe and cheap — no connection is duplicated.

---

## Pagination

Cursor-based. Each page response contains an optional `cursor` field:

- **Present** → more pages exist. Pass as `cursor` in the next call.
- **Absent** → current page is the last.

Cursors are opaque server-side DynamoDB sort keys. Do not parse, construct, or modify them. Do not reuse a cursor from `list` in a `search` call or vice versa.

The `_all` helper methods (`list_all`, `search_all`, `update_all_where`, etc.) automate cursor following. They issue pages sequentially because each cursor depends on the previous response.

```rust
// Manual pagination
let mut cursor: Option<String> = None;
loop {
    let page = ns.list(Some(100), cursor.as_deref()).await?;
    process(&page.keys);
    match page.cursor {
        Some(c) => cursor = Some(c),
        None    => break,
    }
}

// Automatic (collects all at once)
let all_keys = ns.list_all(Some(100)).await?;
```

---

## Merge Semantics

`update_one` and `update_where` perform shallow merges, not full replacements.

### Data merge

| Existing `data` type | Patch `data` type | Result |
|---|---|---|
| JSON object | JSON object | Patch keys overwrite/add; unspecified keys preserved |
| any | non-object OR existing is non-object | Existing replaced entirely by patch |
| any | `None` (omitted) | Existing unchanged |

### `sp` merge

`metadata.sp` is always shallow-merged:
- Patch keys overwrite or add to existing `sp`.
- Unspecified `sp` keys are preserved.
- To delete a specific `sp` key, set it to `null` in the patch.
- To clear all `sp`, use `set()` (full replace) with `sp: Some(Map::new())`.

### Atomicity

Partial updates perform a read-then-write on the server. They are **not atomic**. Under concurrent writes to the same key, the last write wins.
