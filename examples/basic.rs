use flex_db::{BulkCreateItem, FlexDb, SearchFilter};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Record {
    name: String,
    score: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("FLEX_DB_TOKEN").expect("FLEX_DB_TOKEN not set");
    let base_url =
        std::env::var("FLEX_DB_URL").unwrap_or_else(|_| "https://api.flexdb.io".into());

    let client = FlexDb::new(base_url, token);

    // Health check (no auth)
    let health = client.health().await?;
    println!("health: {}", health.status);

    // Namespace-scoped client
    let ns = client.namespace("example-ns");

    // --- Single object operations ---

    // Create
    let key = ns.create(&Record { name: "Alice".into(), score: 42 }, None).await?;
    println!("created: {key}");

    // Read
    let obj = ns.get::<Record>(&key).await?;
    println!("read: {:?} (warm={})", obj.data, obj.metadata.w);

    // Full replace
    let _ = ns.set(&key, &Record { name: "Alice".into(), score: 99 }, None).await?;

    // Partial update (merge patch)
    let sp = serde_json::json!({ "score": 99 });
    let _ = ns
        .update_one::<serde_json::Value>(&key, None, sp.as_object())
        .await?;

    // Delete
    ns.delete(&key).await?;
    println!("deleted {key}");

    // --- Bulk operations ---

    let items: Vec<BulkCreateItem<Record>> = vec![
        BulkCreateItem { data: Record { name: "Bob".into(), score: 10 }, metadata: None },
        BulkCreateItem { data: Record { name: "Carol".into(), score: 20 }, metadata: None },
        BulkCreateItem { data: Record { name: "Dave".into(), score: 10 }, metadata: None },
    ];
    let keys = ns.bulk_create(&items).await?;
    println!("bulk created: {keys:?}");

    // --- Parallel reads ---
    let (r1, r2) = tokio::join!(
        ns.get::<Record>(&keys[0]),
        ns.get::<Record>(&keys[1]),
    );
    println!("parallel read: {:?}, {:?}", r1?.data, r2?.data);

    // --- List ---
    let page = ns.list(Some(10), None).await?;
    println!("list page: {:?}", page.keys);

    // --- Search ---
    let filters = vec![SearchFilter::eq("score", 10u32)];
    let results = ns.search_all(&filters, None).await?;
    println!("search results (score==10): {results:?}");

    // --- Cleanup ---
    ns.bulk_delete(&keys).await?;
    println!("bulk deleted");

    Ok(())
}
