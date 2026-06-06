# Flex SDK Definition

> Reference for building sdk packages for FlexDB.
> All endpoint behaviour described here is derived directly from the Rust server source.

---

## Overview

The SDK wraps the **data API** — all routes under `/v2/`. These are the only routes accessible with a JWT token. Admin/DB-management routes (`/db/*`) are MASTER_TOKEN-only and are never exposed through the public SDK.

**Base URL:** `https://<host>/v2`

**Authentication:** Every request carries `Authorization: Bearer <jwt>` where the JWT is issued via `POST /db/:id/keys` (admin API). The token encodes a `db_id` and a 2-bit `perms` field (1=READ, 2=WRITE, 3=ALL). The server validates the token on every request; no session state is kept.

---

## Key constraints (apply everywhere)

### Key format
All caller-supplied keys must be **nanoid-compatible**:
- Alphabet: `A-Za-z0-9_-`
- Length: 5–21 characters inclusive
- Violation returns `400 ERR_INVALID_KEY`

Auto-generated keys (from `POST /v2/o/:ns` and `POST /v2/bulk/create`) are nanoid(21) and are always valid.

### Namespace
`:ns` is a free-form string that organises keys within a database. It is **not validated** by the server beyond being a non-empty path segment. Think of it as a "collection" or "table" name. Namespaces are implicit — they spring into existence on first write and disappear when all their keys are deleted.

### Object size limit
Maximum serialised JSON `data` size per object: `cold_max_size` (configured per deployment, typically 5 MB). Violation returns `413 ERR_REQUEST_TOO_LARGE`.

### Search property (sp) fields
`sp` is an optional flat map of searchable properties attached to an object. Rules:
- Keys: `A-Za-z0-9_`, max 64 chars, non-empty
- Values: any JSON scalar (string, number, boolean, null)
- `sp` is stored as DynamoDB sub-map and used exclusively by `search` — it is NOT returned in `get` responses
- Default: empty `{}`

---

## Response envelope

Every response, success or error, uses this envelope:

```json
{ "v": "2.6.x", "ok": true,  "data": { ... } }
{ "v": "2.6.x", "ok": false, "error": { "code": "ERR_...", "message": "..." } }
```

`v` is the server version string. The SDK should expose this for diagnostics.

---

## Endpoints

### GET `/v2/o/:ns/:key`
Retrieve a single object.

**Permissions required:** READ (perms bit 1)

**Path params:**
| Param | Type   | Description |
|-------|--------|-------------|
| `ns`  | string | Namespace   |
| `key` | string | Object key (must pass nanoid validation) |

**Response `data`:**
```json
{ "data": <any JSON value that was stored> }
```

**Behaviour:**
- Reads L1 → L2 → KV → Cold in waterfall order; returns first hit.
- On a KV or Cold hit, TierEval may promote the object to L2 asynchronously (not visible to the caller).
- Returns `404 ERR_NOT_FOUND` if no value found at any tier.

**Cost:** Charged based on which tier served the request. L1 hits are free; Cold hits are most expensive.

---

### PUT `/v2/o/:ns/:key`
Set (create or overwrite) a single object.

**Permissions required:** WRITE (perms bit 2)

**Path params:** same as GET

**Request body:**
```json
{
  "data": <any JSON value>,
  "sp":   { "field": <scalar>, ... }   // optional
}
```

**Response `data`:** `{}` (empty, `ok: true`)

**Behaviour:**
- Writes to L2 (Valkey) with a 5-minute write-buffer TTL.
- Permanently committed to KV or Cold by `WriteBufferFlushJob` within ~60 seconds.
- Object is immediately readable via GET (served from L2) but will NOT appear in `list` or `search` until the flush completes.
- Overwrites any existing object at the same key.
- Cost charged before the write (L2 write cost, scaled by payload size).

---

### POST `/v2/o/:ns`
Create an object with a server-generated key.

**Permissions required:** WRITE

**Path params:**
| Param | Type   | Description |
|-------|--------|-------------|
| `ns`  | string | Namespace   |

**Request body:**
```json
{
  "data": <any JSON value>,
  "sp":   { ... }   // optional
}
```

**Response `data`:**
```json
{ "id": "V1StGXR8_Z5jdHi6B-myT" }
```

**Behaviour:**
- Generates a nanoid(21) key server-side; caller never supplies a key.
- Otherwise identical to PUT: write-buffer path, ~60s flush lag before appearing in list/search.

---

### DELETE `/v2/o/:ns/:key`
Delete a single object.

**Permissions required:** WRITE

**Path params:** same as GET

**Response `data`:** `{}` (empty, `ok: true`)

**Behaviour:**
- Parallel delete across L2 data, L2 meta, KV data, KV meta, and S3.
- Also deregisters from the write-buffer index (handles objects that haven't flushed yet).
- Storage pre-credits are refunded for remaining TTL on each tier.
- Does NOT return an error if the key does not exist (idempotent delete).

---

### GET `/v2/list/:ns`
List keys (or key+value pairs) in a namespace.

**Permissions required:** READ

**Query params:**
| Param     | Type    | Default | Max  | Description |
|-----------|---------|---------|------|-------------|
| `limit`   | integer | 50      | 1000 | Max items to return |
| `cursor`  | string  | null    | —    | Pagination cursor from previous response |
| `hydrate` | boolean | false   | —    | When true, fetch and return full object data alongside keys |

**Response `data` (hydrate=false):**
```json
{ "keys": ["key1", "key2", ...], "cursor": "next-cursor-or-null" }
```

**Response `data` (hydrate=true):**
```json
{
  "items": [
    { "key": "key1", "data": <value> },
    ...
  ],
  "cursor": "next-cursor-or-null"
}
```

**Behaviour:**
- Queries DynamoDB `meta:` rows — only objects that have been flushed from the write-buffer appear here. Objects written within the last ~60 seconds may not appear.
- `cursor` is opaque; pass it verbatim to the next call. A null cursor means no more pages.
- `hydrate=true` runs a `bulk_get` waterfall after the DynamoDB query; values may be served from any tier.

---

### POST `/v2/search/:ns`
Search objects in a namespace by searchable properties.

**Permissions required:** READ

**Request body:**
```json
{
  "filters": [
    { "field": "status", "op": "eq",  "value": "active" },
    { "field": "score",  "op": "gte", "value": 80 }
  ],
  "limit":   50,
  "cursor":  null,
  "hydrate": false
}
```

**`filters` — required, non-empty array. Each filter:**
| Field   | Type   | Description |
|---------|--------|-------------|
| `field` | string | `sp` property name (same rules as sp keys) |
| `op`    | string | One of: `eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `contains`, `starts_with` |
| `value` | scalar | string, number, boolean, or null |

**`limit`** (integer, default 50, max 1000)  
**`cursor`** (string, optional — opaque pagination token)  
**`hydrate`** (boolean, default false)

**Response `data` (hydrate=false):**
```json
{ "keys": ["key1", ...], "cursor": "next-or-null" }
```

**Response `data` (hydrate=true):**
```json
{
  "items": [
    { "key": "key1", "data": <value> }
  ],
  "cursor": "next-or-null"
}
```

**Behaviour:**
- Always queries DynamoDB with a `FilterExpression` on the `sp.*` sub-map.
- At least one filter is required (`400 ERR_MISSING_FILTER` otherwise).
- Only objects that have been flushed to DynamoDB appear. Same ~60s lag as list.
- `hydrate=true` follows matching keys through the full `bulk_get` waterfall.

---

### POST `/v2/bulk/get`
Fetch multiple objects in a single request.

**Permissions required:** READ

**Request body:**
```json
{
  "ns":   "collection-name",
  "keys": ["key1", "key2", "key3"]
}
```

**Constraints:**
- `keys` max length: `BULK_GET_LIMIT` (configured per deployment, default 100)
- All keys must pass nanoid validation — one invalid key rejects the entire request with `400 ERR_INVALID_KEY`
- Exceeding the limit: `413 ERR_BULK_TOO_LARGE`

**Response `data`:**
```json
{
  "items": [
    { "key": "key1", "ok": true,  "data": <value> },
    { "key": "key2", "ok": false }
  ]
}
```

**Behaviour:**
- Per-item waterfall: L1 → L2 → KV → Cold for each key.
- Preserves input order in the response array.
- Missing keys are `ok: false` (no error code per item — check at the top level for auth/limit errors).

---

### POST `/v2/bulk/set`
Set multiple objects in a single request.

**Permissions required:** WRITE

**Request body:**
```json
{
  "ns": "collection-name",
  "items": [
    { "key": "key1", "data": <value>, "sp": { ... } },
    { "key": "key2", "data": <value> }
  ]
}
```

**Constraints:**
- `items` max length: `BULK_SET_LIMIT`
- All keys must pass nanoid validation
- Each `data` must be under `cold_max_size`
- Total cost (sum of all item costs) is charged **before** any writes. Partial failures still consume quota.

**Response `data`:**
```json
{
  "items": [
    { "ok": true },
    { "ok": false, "error": "..." }
  ]
}
```

**Behaviour:**
- Each item goes through the same write-buffer path as a single PUT.
- Items are fanned out concurrently (up to `BULK_SET_CONCURRENCY`).
- Response order matches input order.

---

### POST `/v2/bulk/create`
Create multiple objects with server-generated keys.

**Permissions required:** WRITE

**Request body:**
```json
{
  "ns": "collection-name",
  "items": [
    { "data": <value>, "sp": { ... } },
    { "data": <value> }
  ]
}
```

**Constraints:**
- `items` max length: `BULK_CREATE_LIMIT`
- Each `data` must be under `cold_max_size`
- Cost charged before any writes

**Response `data`:**
```json
{
  "items": [
    { "ok": true,  "id": "V1StGXR8_Z5jdHi6B-myT" },
    { "ok": false, "error": "..." }
  ]
}
```

**Behaviour:**
- Each item gets a nanoid(21) key generated server-side.
- Write-buffer path, same ~60s flush lag.
- Slightly cheaper than bulk/set per item (no L2 invalidation DEL needed since keys are new).

---

### POST `/v2/bulk/delete`
Delete multiple objects in a single request.

**Permissions required:** WRITE

**Request body:**
```json
{
  "ns":   "collection-name",
  "keys": ["key1", "key2"]
}
```

**Constraints:**
- `keys` max length: `BULK_DELETE_LIMIT`
- All keys must pass nanoid validation

**Response `data`:**
```json
{
  "items": [
    { "ok": true },
    { "ok": false, "error": "..." }
  ]
}
```

**Behaviour:**
- Parallel delete across all tiers for each key.
- Missing keys do not error (idempotent).

---

## Error codes

| Code                       | HTTP | When                                                   |
|----------------------------|------|--------------------------------------------------------|
| `ERR_MISSING_AUTH`         | 401  | No `Authorization` header                             |
| `ERR_UNAUTHORIZED`         | 401  | Token invalid, expired, or revoked                    |
| `ERR_PERMISSION_DENIED`    | 403  | Token lacks required permission bit                   |
| `ERR_NOT_FOUND`            | 404  | Object does not exist at any tier                     |
| `ERR_MISSING_FILTER`       | 400  | Search called with empty `filters` array              |
| `ERR_INVALID_KEY`          | 400  | Key fails nanoid constraint                            |
| `ERR_RATE_LIMIT_SECOND`    | 429  | Per-second RPS cap exceeded                           |
| `ERR_RATE_LIMIT_MONTH`     | 429  | Monthly budget (call cost + storage) exhausted        |
| `ERR_REQUEST_TOO_LARGE`    | 413  | Single object `data` exceeds `cold_max_size`          |
| `ERR_BULK_TOO_LARGE`       | 413  | Bulk request item count exceeds the operation's limit |
| `ERR_UNPROCESSABLE_ENTITY` | 422  | Request body not valid JSON or wrong shape            |
| `ERR_INTERNAL`             | 500  | Unexpected server error                               |

---

## Rate limiting & quotas

**Per-second (RPS):** Enforced via Lua INCR+EXPIRE in Redis. Limit = `RPS_BY_KIND[db.kind] * db.double_down`.
- Default by kind: Webshop 200, IoT 500, CMS 150, SaaS 150, Analytics 50, General/Custom 100.
- Returns `429 ERR_RATE_LIMIT_SECOND` immediately.

**Monthly budget:** `€5.00 * db.double_down` per calendar month. Budget = call costs + storage pre-credits. Returns `429 ERR_RATE_LIMIT_MONTH` when exhausted.

**Response headers** (always present on rate-limited responses):
The server may add `X-RateLimit-*` headers — treat these as informational.

---

## Write-buffer visibility lag

Writes (PUT, POST create, bulk set/create) land in L2 (Valkey) immediately with a 5-minute TTL. The `WriteBufferFlushJob` commits them to permanent storage (DynamoDB/S3) every ~60 seconds.

**SDK contract:**
- GET: immediate (reads from L2/L1)
- DELETE: immediate (deregisters from write-buffer)
- LIST, SEARCH: up to ~60s lag for newly written objects

This is intentional. Document it in the SDK and expose it as a known behaviour, not a bug.

---

## SDK design guidance

### Client lifecycle
One client instance per application. The client holds:
- Base URL
- JWT token (or a function that returns one, for token refresh)
- Optional namespace binding (bind to a namespace to skip passing `ns` on every call)

### Pagination
`list` and `search` return a `cursor`. SDK should expose:
- Single-page call returning `{ items, cursor }`
- Async iterator / async generator that paginates until `cursor` is null

### Bulk operations
Expose bulk variants separately from single-object variants. Do not auto-batch — let the caller decide.

### Type safety
The SDK is TypeScript-first. `data` is typed as `unknown` or generic `T`. `sp` values are `string | number | boolean | null`.

### Retry logic
Retry on `5xx` and network errors with exponential backoff. Do NOT retry on `4xx` (client errors) or `429` (rate limit — surface these to the caller).

### Zero dependencies
The SDK must use only native `fetch` and standard JS globals. No `axios`, no `node-fetch`.
