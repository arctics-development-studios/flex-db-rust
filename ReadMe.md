# flex-db

Rust SDK for the [Flex DB](https://flexdb.io) API — a high-performance
Database-as-a-Service with automatic storage tiering, search, and bulk operations.

## Features

- Async/await with `tokio` — all methods take `&self` so any number of requests can run concurrently
- Fully typed generics — bring your own `serde::Serialize`/`Deserialize` structs; no forced `serde_json::Value`
- Pagination helpers — single-page methods plus `_all` variants that follow cursors automatically
- Typed error handling — every API error code maps to a Rust enum variant
- Minimal dependencies — `reqwest`, `serde`, `serde_json`, `thiserror`

## Installation

```toml
[dependencies]
flex-db = "2.6"
tokio   = { version = "1", features = ["rt-multi-thread", "macros"] }
serde   = { version = "1", features = ["derive"] }
```

## Quick Start

```rust
use flex_db::{FlexDb, SearchFilter};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct User {
    name: String,
    age: u32,
}

#[tokio::main]
async fn main() -> Result<(), flex_db::Error> {
    let client = FlexDb::new("https://api.flexdb.io", "your-jwt-token");
    let ns = client.namespace("users");

    // Create with auto-generated key
    let key = ns.create(&User { name: "Alice".into(), age: 30 }, None).await?;
    println!("created: {key}");

    // Read
    let obj = ns.get::<User>(&key).await?;
    println!("{:?}", obj.data);

    // Replace (upsert at known key)
    ns.set(&key, &User { name: "Alice".into(), age: 31 }, None).await?;

    // Delete
    ns.delete(&key).await?;

    Ok(())
}
```

## Authentication

The `token` parameter is a raw RS256 JWT issued by Flex DB. Pass it without the `Bearer ` prefix — the SDK adds that for you.

```rust
let token = std::env::var("FLEX_DB_TOKEN").expect("FLEX_DB_TOKEN not set");
let client = FlexDb::new("https://api.flexdb.io", token);
```

The token encodes a `db_id` and a 2-bit `perms` field:
- `1` — READ only
- `2` — WRITE only
- `3` — READ + WRITE

## Namespaces

Every data operation requires a namespace. Objects with the same key in different namespaces are completely independent.

```rust
let users  = client.namespace("users");
let events = client.namespace("events");
```

`Namespace` is `Clone + Send + Sync` — clone freely and pass across tasks. Namespaces are implicit: they spring into existence on first write and disappear when all their keys are deleted.

## Key Constraints

Caller-supplied keys must use the nanoid alphabet (`A-Za-z0-9_-`) and be between 5 and 21 characters long. Violation returns `Error::Api { code: ApiErrorCode::InvalidKey, .. }`.

Auto-generated keys (from `create` and `bulk_create`) are always valid nanoid(21) strings.

## CRUD Operations

```rust
// Create (auto-generated key)
let key = ns.create(&my_struct, None).await?;

// Create with search properties
let sp = serde_json::json!({ "status": "active", "score": 42 });
let key = ns.create(&my_struct, sp.as_object()).await?;

// Read
let obj: flex_db::GetResponse<MyType> = ns.get(&key).await?;
println!("{:?}", obj.data);

// Full replace (upsert)
ns.set(&key, &updated_struct, None).await?;

// Delete (always Ok, even if key is absent — idempotent)
ns.delete(&key).await?;
```

## Search

Filter objects by their stored search properties (`sp`). All filters are AND-ed together.

`sp` fields are attached at write time and are **not** returned by `get`. They exist solely to support search queries.

```rust
use flex_db::SearchFilter;

let filters = vec![
    SearchFilter::eq("status", "active"),
    SearchFilter::gte("score", 10u32),
    SearchFilter::starts_with("label", "prod-"),
    SearchFilter::contains("tag", "rust"),
];

// One page of matching keys
let page = ns.search(&filters, Some(50), None).await?;

// All matching keys (follows cursors automatically)
let all_keys = ns.search_all(&filters, None).await?;

// With full object data
let hydrated = ns.search_full::<MyType>(&filters, None, None).await?;
for item in hydrated.items {
    println!("{}: {:?}", item.key, item.data);
}
```

### Available filter operators

| Constructor | Operator |
|---|---|
| `SearchFilter::eq(field, value)` | equals |
| `SearchFilter::neq(field, value)` | not equals |
| `SearchFilter::gt(field, value)` | greater than |
| `SearchFilter::gte(field, value)` | greater than or equal |
| `SearchFilter::lt(field, value)` | less than |
| `SearchFilter::lte(field, value)` | less than or equal |
| `SearchFilter::contains(field, substring)` | contains substring |
| `SearchFilter::starts_with(field, prefix)` | starts with prefix |

## Listing Objects

```rust
// One page (keys only)
let page = ns.list(Some(100), None).await?;
println!("{} keys, has_more={}", page.keys.len(), page.cursor.is_some());

// All keys across all pages
let all = ns.list_all(None).await?;

// With full object data
let full = ns.list_full::<MyType>(Some(50), None).await?;
for item in full.items {
    println!("{}: {:?}", item.key, item.data);
}
```

## Bulk Operations

Bulk operations are processed concurrently on the server. Each operation returns per-item results in the same order as the input.

```rust
use flex_db::{BulkCreateItem, BulkSetItem};

// Bulk create (auto-generated keys)
let items = vec![
    BulkCreateItem { data: MyType { .. }, sp: None },
    BulkCreateItem { data: MyType { .. }, sp: None },
];
let result = ns.bulk_create(&items).await?;
let keys: Vec<String> = result.items
    .iter()
    .filter_map(|i| i.id.clone())
    .collect();

// Bulk get
let fetched = ns.bulk_get::<MyType>(&keys).await?;
for item in &fetched.items {
    if item.ok {
        println!("{}: {:?}", item.key, item.data);
    }
}

// Bulk upsert (caller-supplied keys)
let updates = vec![
    BulkSetItem { key: keys[0].clone(), data: updated, sp: None },
];
ns.bulk_set(&updates).await?;

// Bulk delete
ns.bulk_delete(&keys).await?;
```

## Write-Buffer Visibility Lag

Writes (create, set, bulk_create, bulk_set) land in L2 cache immediately:

| Operation | Visible immediately? |
|-----------|----------------------|
| `get` | Yes — reads from L2/L1 |
| `delete` | Yes — deregisters from write buffer |
| `list`, `search` | Up to ~60 s lag while write buffer flushes |

This is by design. Objects are committed to DynamoDB by a background flush job every ~60 seconds.

## Parallel Requests

Because all methods take `&self`, you can fire multiple concurrent requests with `tokio::join!`:

```rust
// Concurrent reads
let (a, b, c) = tokio::join!(
    ns.get::<MyType>("key1"),
    ns.get::<MyType>("key2"),
    ns.get::<MyType>("key3"),
);

// Concurrent across namespaces
let ns1 = client.namespace("users");
let ns2 = client.namespace("sessions");
let (users, sessions) = tokio::join!(
    ns1.list_all(None),
    ns2.list_all(None),
);
```

## Error Handling

```rust
use flex_db::{ApiErrorCode, Error};

match ns.get::<MyType>("some-key").await {
    Ok(obj) => println!("{:?}", obj.data),
    Err(Error::Api { code: ApiErrorCode::NotFound, .. }) => {
        println!("not found");
    }
    Err(Error::Api { code: ApiErrorCode::RateLimitSecond, .. }) => {
        // back off and retry
    }
    Err(Error::Api { code: ApiErrorCode::RateLimitMonth, .. }) => {
        // monthly budget exhausted — surface to caller
    }
    Err(Error::Api { code, message }) => {
        eprintln!("api error {code:?}: {message}");
    }
    Err(Error::Transport(e)) => {
        eprintln!("network error: {e}");
    }
    Err(e) => eprintln!("other: {e}"),
}
```

Always match on `ApiErrorCode` variants — error messages may change between server versions, but codes are stable.

### Error codes

| Variant | HTTP | When |
|---|---|---|
| `MissingAuth` | 401 | No `Authorization` header |
| `Unauthorized` | 401 | Token invalid, expired, or revoked |
| `PermissionDenied` | 403 | Token lacks READ or WRITE permission |
| `NotFound` | 404 | Object does not exist at any tier |
| `MissingFilter` | 400 | Search called with empty `filters` |
| `InvalidKey` | 400 | Key fails the nanoid constraint |
| `RateLimitSecond` | 429 | Per-second RPS cap exceeded |
| `RateLimitMonth` | 429 | Monthly budget exhausted |
| `RequestTooLarge` | 413 | Object `data` exceeds the size limit |
| `BulkTooLarge` | 413 | Bulk item count exceeds the limit |
| `UnprocessableEntity` | 422 | Request body not valid JSON or wrong shape |
| `Internal` | 500 | Unexpected server error |

Do **not** retry on `4xx` errors. For `429`, surface it to the caller rather than retrying immediately.

## Pagination

Single-page methods return a `cursor` field that is `Some` when more results exist:

```rust
let mut cursor: Option<String> = None;
loop {
    let page = ns.list(Some(100), cursor.as_deref()).await?;
    for key in &page.keys {
        process(key);
    }
    match page.cursor {
        Some(c) => cursor = Some(c),
        None    => break,
    }
}
```

Or use the `_all` helpers to collect everything automatically:

```rust
let all_keys = ns.list_all(Some(100)).await?;
let all_users = ns.list_all_full::<User>(None).await?;
```

## Health Check

```rust
let health = client.health().await?;
assert_eq!(health.status, "healthy");
```

No authentication required.
