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

    // Health check (no auth required)
    let health = client.health().await?;
    println!("health: {}", health.status);

    let ns = client.namespace("example-ns");

    // --- Single-object operations ---

    // Create with auto-generated key
    let key = ns.create(&Record { name: "Alice".into(), score: 42 }, None).await?;
    println!("created: {key}");

    // Read
    let obj = ns.get::<Record>(&key).await?;
    println!("read: {:?}", obj.data);

    // Full replace (upsert)
    ns.set(&key, &Record { name: "Alice".into(), score: 99 }, None).await?;
    println!("updated {key}");

    // Delete (idempotent — always Ok even if key is absent)
    ns.delete(&key).await?;
    println!("deleted {key}");

    // --- Bulk operations ---

    let items: Vec<BulkCreateItem<Record>> = vec![
        BulkCreateItem { data: Record { name: "Bob".into(), score: 10 }, sp: None },
        BulkCreateItem { data: Record { name: "Carol".into(), score: 20 }, sp: None },
        BulkCreateItem { data: Record { name: "Dave".into(), score: 10 }, sp: None },
    ];
    let result = ns.bulk_create(&items).await?;
    let keys: Vec<String> = result.items.iter().filter_map(|i| i.id.clone()).collect();
    println!("bulk created: {keys:?}");

    // Bulk get
    let fetched = ns.bulk_get::<Record>(&keys).await?;
    for item in &fetched.items {
        if item.ok {
            println!("  got {}: {:?}", item.key, item.data);
        }
    }

    // Parallel reads with tokio::join!
    let (r1, r2) = tokio::join!(
        ns.get::<Record>(&keys[0]),
        ns.get::<Record>(&keys[1]),
    );
    println!("parallel read: {:?}, {:?}", r1?.data, r2?.data);

    // --- List (keys only, non-hydrated) ---
    let page = ns.list(Some(10), None).await?;
    println!("list page: {:?}", page.keys);

    // --- Search by search properties ---
    // Note: sp must be set at write time to be queryable. This example assumes
    // objects were created with sp = { "score": <number> }.
    let filters = vec![SearchFilter::eq("score", 10u32)];
    let results = ns.search_all(&filters, None).await?;
    println!("search results (score==10): {results:?}");

    // --- Cleanup ---
    let delete_result = ns.bulk_delete(&keys).await?;
    let deleted = delete_result.items.iter().filter(|i| i.ok).count();
    println!("bulk deleted {deleted}/{} keys", keys.len());

    Ok(())
}
