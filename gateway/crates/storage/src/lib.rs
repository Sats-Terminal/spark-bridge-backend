use std::sync::{Arc, Mutex};

use sqlx::PgPool;

use crate::errors::DatabaseError;

pub mod errors;
pub mod keys;
pub mod models;
pub mod request;
pub mod traits;

#[derive(Debug, Clone)]
pub struct Storage {
    pool: Arc<Mutex<PgPool>>,
}

impl Storage {
    pub async fn new(url: String) -> Result<Self, DatabaseError> {
        let pool = PgPool::connect(&url)
            .await
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        Ok(Self {
            pool: Arc::new(Mutex::new(pool)),
        })
    }
}
