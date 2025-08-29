use sqlx::PgPool;
use std::sync::Arc;
use std::sync::Mutex;
use crate::errors::DatabaseError;

pub mod models;
pub mod traits;
pub mod request;
pub mod errors;
pub mod keys;

#[derive(Debug, Clone)]
pub struct Storage {
    pool: Arc<Mutex<PgPool>>,
}

impl Storage {
    pub async fn new(url: String) -> Result<Self, DatabaseError> {
        let pool = PgPool::connect(&url).await.map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        Ok(Self { pool: Arc::new(Mutex::new(pool)) })
    }
}
