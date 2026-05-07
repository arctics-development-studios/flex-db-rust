//! # flex-db
//!
//! Rust SDK for the [Flex DB](https://flexdb.io) API.
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
//!     // Create
//!     let key = ns.create(&User { name: "Alice".into(), age: 30 }, None).await?;
//!
//!     // Read
//!     let obj = ns.get::<User>(&key).await?;
//!     println!("{:?}", obj.data);
//!
//!     // Search
//!     let page = ns.search(&[SearchFilter::eq("age", 30u32)], None, None).await?;
//!     println!("{:?}", page.keys);
//!
//!     Ok(())
//! }
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
    BulkCreateItem, BulkSetItem, GetResponse, HealthResponse, HydratedItem, ListPage,
    ListPageFull, ObjectMeta, UpdateFilterResponse, WriteMetadata,
};
