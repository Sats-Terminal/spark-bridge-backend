use sqlx::PgPool;
use sqlx::types::Uuid;
use tokio;
use std::sync::Arc;
use std::sync::Mutex;

pub mod models;
pub mod traits;
pub mod request;
pub mod errors;
pub mod keys;

pub struct Storage {
    pool: Arc<Mutex<PgPool>>,
}

impl Storage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool: Arc::new(Mutex::new(pool)) }
    }
}
