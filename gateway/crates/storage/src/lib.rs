use sqlx::PgPool;
use sqlx::types::Uuid;
use tokio;
use std::sync::Arc;
use std::sync::Mutex;

pub mod models;
pub mod traits;
pub mod request;
pub mod errors;

struct Storage {
    pool: Arc<Mutex<PgPool>>,
}

impl Storage {
    fn new(pool: PgPool) -> Self {
        Self { pool: Arc::new(Mutex::new(pool)) }
    }
}

#[tokio::test]
async fn test_storage() {
    let url = "postgres://postgres:postgres@localhost:5433/postgres";
    let pool: PgPool = PgPool::connect(url).await.unwrap();
    let request_id = Uuid::new_v4();
    
    let result = sqlx::query!(
        "SELECT * FROM requests WHERE request_id = $1",
        request_id
    ).fetch_all(&pool).await.unwrap();
    
    println!("{:?}", result);
    
}