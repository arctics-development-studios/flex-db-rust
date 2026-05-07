# flex-db

Rust SDK for the [Flex DB](https://flexdb.io) API — a high-performance Database-as-a-Service with automatic storage tiering, search, and bulk operations.

## Features

- Async/await with `tokio` — all methods take `&self` so any number of requests can run concurrently
- Fully typed generics — bring your own `serde::Serialize`/`Deserialize` structs; no forced `serde_json::Value`
- Pagination helpers — single-page methods plus `_all` variants that follow cursors automatically
- Typed error handling — every API error code maps to a Rust enum variant
- Minimal dependencies — `reqwest`, `serde`, `serde_json`, `thiserror`

## Installation

```toml
[dependencies]
flex-db = "2.2.0"
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

    // Create
    let key = ns.create(&User { name: "Alice".into(), age: 30 }, None).await?;
    println!("created: {key}");

    // Read
    let obj = ns.get::<User>(&key).await?;
    println!("{:?}", obj.data);

    // Update (shallow merge)
    ns.update_one::<serde_json::Value>(&key, None, None).await?;

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

## Namespaces

Every data operation requires a namespace. Objects with the same key in different namespaces are completely independent.

```rust
let users  = client.namespace("users");
let events = client.namespace("events");
```

`Namespace` is `Clone + Send + Sync` — clone freely and pass across tasks.

## CRUD Operations

```rust
// Create (auto-generated key)
let key = ns.create(&my_struct, None).await?;

// Create with search parameters
let sp = serde_json::json!({ "status": "active", "score": 42 });
let key = ns.create(&my_struct, sp.as_object()).await?;

// Read
let obj: flex_db::GetResponse<MyType> = ns.get(&key).await?;
println!("warm tier: {}", obj.metadata.w);

// Full replace (upsert)
ns.set(&key, &updated_struct, None).await?;

// Partial update — only specified fields are changed
let patch = serde_json::json!({ "score": 99 });
ns.update_one::<serde_json::Value>(&key, Some(&patch), None).await?;

// Delete (always Ok, even if key absent)
ns.delete(&key).await?;
```

## Search

Filter objects by their stored search parameters (`metadata.sp`). All filters are AND-ed.

```rust
use flex_db::SearchFilter;

let filters = vec![
    SearchFilter::eq("status", "active"),
    SearchFilter::gte("score", 10u32),
    SearchFilter::starts_with("label", "prod-"),
];

// One page of matching keys
let page = ns.search(&filters, Some(50), None).await?;

// All matching keys (follows cursors automatically)
let all_keys = ns.search_all(&filters, None).await?;

// With full object data
let hydrated = ns.search_full::<MyType>(&filters, None, None).await?;
for item in hydrated.keys {
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
| `SearchFilter::starts_with(field, prefix)` | starts with |
| `SearchFilter::exists(field)` | field is present |

## Listing Objects

```rust
// One page
let page = ns.list(Some(100), None).await?;
println!("{} keys, has_more={}", page.keys.len(), page.cursor.is_some());

// All pages collected
let all = ns.list_all(None).await?;

// With full object data
let full = ns.list_full::<MyType>(Some(50), None).await?;
```

## Bulk Operations

Bulk operations are processed concurrently on the server (up to 50 items per call).

```rust
use flex_db::BulkCreateItem;

// Bulk create
let items = vec![
    BulkCreateItem { data: MyType { .. }, metadata: None },
    BulkCreateItem { data: MyType { .. }, metadata: None },
];
let keys = ns.bulk_create(&items).await?;

// Bulk upsert
use flex_db::BulkSetItem;
let updates = vec![
    BulkSetItem { key: keys[0].clone(), data: updated, metadata: None },
];
ns.bulk_set(&updates).await?;

// Bulk delete
ns.bulk_delete(&keys).await?;
```

## Filter-Based Bulk Update

Update all objects matching a filter in one call (or follow cursors for large sets).

```rust
let filters = vec![SearchFilter::eq("status", "pending")];
let patch = serde_json::json!({ "status": "processed" });
let sp_patch = serde_json::json!({ "status": "processed" });

// All matching objects, all pages
let total_updated = ns.update_all_where(
    &filters,
    Some(&patch),
    sp_patch.as_object(),
    None,
).await?;
println!("updated {total_updated} objects");
```

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
    Err(Error::Api { code, message }) => {
        eprintln!("api error {code:?}: {message}");
    }
    Err(Error::Transport(e)) => {
        eprintln!("network error: {e}");
    }
    Err(e) => eprintln!("other: {e}"),
}
```

Always match on `ApiErrorCode` variants — error messages may change between server versions.

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
```

## Object Metadata

Every object read returns its `ObjectMeta`:

```rust
let obj = ns.get::<MyType>(&key).await?;
let meta = &obj.metadata;

println!("warm tier: {}", meta.w);          // true=DynamoDB, false=S3
println!("size bytes: {}", meta.s);
println!("last updated: {}", meta.lut);     // Unix timestamp
println!("search params: {:?}", meta.sp);
```

Storage tier is assigned automatically based on object size and access frequency. Objects ≤ 50 KB go to DynamoDB (warm); larger objects go to S3 (cold). Hot objects are promoted to a Valkey cache automatically.

## Health Check

```rust
let health = client.health().await?;
assert_eq!(health.status, "healthy");
```

No authentication required.

## Full API Reference

See [sdk_definition.md](sdk_definition.md) for the complete method signatures, all type definitions, and behavioral details.
