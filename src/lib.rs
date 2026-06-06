//! # flex-db
//!
//! Rust SDK for the [Flex DB](https://flexdb.io) API — a high-performance
//! Database-as-a-Service with automatic storage tiering, search, and bulk operations.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use flex_db::{FlexDb, SearchFilter};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Serialize, Deserialize)]
//! struct User { name: String, age: u32 }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), flex_db::Error> {
//!     let client = FlexDb::new("https://api.flexdb.io", "your-jwt");
//!     let ns = client.namespace("users");
//!
//!     // Create with auto-generated key
//!     let key = ns.create(&User { name: "Alice".into(), age: 30 }, None).await?;
//!
//!     // Read
//!     let obj = ns.get::<User>(&key).await?;
//!     println!("{:?}", obj.data);
//!
//!     // Search by stored properties
//!     let hits = ns.search(&[SearchFilter::eq("age", 30u32)], None, None).await?;
//!     println!("{:?}", hits.keys);
//!
//!     // Delete
//!     ns.delete(&key).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Key concepts
//!
//! ### Authentication
//!
//! Pass a raw RS256 JWT (without the `Bearer ` prefix) to [`FlexDb::new`].
//! The token encodes a `db_id` and a 2-bit `perms` field (1=READ, 2=WRITE, 3=ALL).
//!
//! ### Namespaces
//!
//! Every data operation is scoped to a namespace — think "collection" or "table".
//! Namespaces are implicit: they spring into existence on first write and disappear
//! when all their keys are deleted.
//!
//! ```rust,no_run
//! # use flex_db::FlexDb;
//! # let client = FlexDb::new("https://api.flexdb.io", "token");
//! let users  = client.namespace("users");
//! let events = client.namespace("events");
//! ```
//!
//! ### Key constraints
//!
//! Caller-supplied keys must use the nanoid alphabet (`A-Za-z0-9_-`) and be
//! 5–21 characters long. Violation returns
//! [`ApiErrorCode::InvalidKey`](crate::ApiErrorCode::InvalidKey).
//! Auto-generated keys (from [`create`](crate::Namespace::create) and
//! [`bulk_create`](crate::Namespace::bulk_create)) are always valid nanoid(21) strings.
//!
//! ### Search properties (`sp`)
//!
//! `sp` is an optional flat map of searchable properties attached to a write.
//! Keys follow `A-Za-z0-9_` (max 64 chars); values are JSON scalars.
//! `sp` is **not** returned by `get` — it exists solely to support `search` filters.
//!
//! ### Write-buffer visibility lag
//!
//! Writes land in L2 cache immediately and are readable via `get` right away.
//! They do **not** appear in `list` or `search` for up to ~60 seconds while
//! the write-buffer flush job commits them to DynamoDB. This is by design.
//!
//! ### Error handling
//!
//! All async methods return `Result<T, `[`Error`]`>`. Match on
//! [`ApiErrorCode`] variants — error messages may change between server releases.
//!
//! ```rust,no_run
//! use flex_db::{ApiErrorCode, Error};
//!
//! # async fn example(ns: flex_db::Namespace) -> Result<(), flex_db::Error> {
//! match ns.get::<serde_json::Value>("my-key").await {
//!     Ok(obj)  => println!("{:?}", obj.data),
//!     Err(Error::Api { code: ApiErrorCode::NotFound, .. }) => println!("not found"),
//!     Err(e)   => eprintln!("error: {e}"),
//! }
//! # Ok(()) }
//! ```

mod client;
mod error;
mod filter;
mod namespace;
mod types;

pub use client::FlexDb;
pub use error::{ApiErrorCode, Error, Result};
pub use filter::{FilterOp, SearchFilter};
pub use namespace::Namespace;
pub use types::{
    BulkCreateItem, BulkCreateResult, BulkCreateResultItem, BulkGetItem, BulkGetResponse,
    BulkSetItem, BulkWriteItem, BulkWriteResponse, GetResponse, HealthResponse, HydratedItem,
    ListPage, ListPageFull,
};
