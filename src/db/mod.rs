pub mod models;

use std::sync::Arc;

use async_once::AsyncOnce;
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::Client;

// Do not, and I repeat, DO NOT try to replace the tokio mutexes with
// a parking_lot or std mutex. It will not work. It will hang with mongodb operations
// indefinitely. I have no idea why. Blame MongoDB.
pub type TokioMutexCache<K, V> = tokio::sync::Mutex<LruCache<K, V>>;
// The reason this is an option is to allow for existing Arcs to be invalidated
// if a breaking change were to occur such as renaming or the name field
// or deleting said thing from the database. Dropping it from the cache
// is not enough because already existing arcs will still be valid.
pub type ArcTokioRwLockOption<T> = Arc<tokio::sync::RwLock<Option<T>>>;
pub type ArcTokioMutexOption<T> = Arc<tokio::sync::Mutex<Option<T>>>;

lazy_static! {
    pub static ref CLIENT: AsyncOnce<Client> = AsyncOnce::new(async {
        let uri = std::env::var("MONGO_URI").expect("MONGO_URI must be set");
        Client::with_uri_str(&uri).await.unwrap() // Nothing works if this fails
    });
}

#[tokio::test]
async fn test_new_client() {
    use dotenv::dotenv;
    dotenv().ok();
    let uri = std::env::var("MONGO_URI").expect("MONGO_URI must be set");
    let _ = Client::with_uri_str(&uri).await.unwrap();
}

pub mod uniques;

/// Simply prepare the database for use.
/// Environment variables must be set for this to work and
/// the `MongoDB` service must be running.
///
/// # Panics
///
/// Panics if the `MongoDB` service is not running or if
/// the environment variables are not set, or if any
/// `MongoDB` error occurs.
pub async fn init() {
    let db = CLIENT.get().await.database("conebot");
    let collections = match db.list_collection_names(None).await {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error when getting collections: {e}");
            panic!();
        }
    };
    let mut columns = vec![
        "currencies".to_owned(),
        "items".to_owned(),
        "dropTables".to_owned(),
        "storeEntries".to_owned(),
        "balances".to_owned(),
        "inventories".to_owned()
    ];
    collections.into_iter().for_each(|coll| columns.retain(|x| x != &coll));

    for coll in columns {
        match db.create_collection(&coll, None).await {
            Ok(_) => println!("Created collection {coll }"),
            Err(e) => {
                eprintln!("Error when creating collection {coll}: {e}");
                panic!();
            }
        }
    }
}
