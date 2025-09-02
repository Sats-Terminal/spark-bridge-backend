use sqlx;
use uuid::Uuid;

use crate::errors::DatabaseError;
use persistent_storage::init::PostgresRepo;

#[derive(Debug, Clone)]
pub struct Key {
    pub key_id: Uuid,
}

#[async_trait::async_trait]
pub trait KeyStorage {
    async fn insert_key(&self, key: Key) -> Result<(), DatabaseError>;

    async fn get_key(&self, key_id: Uuid) -> Result<Key, DatabaseError>;
}

#[async_trait::async_trait]
impl KeyStorage for PostgresRepo {
    async fn insert_key(&self, key: Key) -> Result<(), DatabaseError> {
        let _ = sqlx::query("INSERT INTO gateway.keys (key_id) VALUES ($1)")
            .bind(key.key_id)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn get_key(&self, key_id: Uuid) -> Result<Key, DatabaseError> {
        let result: (Uuid,) = sqlx::query_as("SELECT * FROM gateway.keys WHERE key_id = $1 LIMIT 1")
            .bind(key_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => DatabaseError::NotFound(e.to_string()),
                _ => DatabaseError::BadRequest(e.to_string()),
            })?;

        Ok(Key { key_id: result.0 })
    }
}
