use sqlx;
use uuid::Uuid;

use crate::{errors::DatabaseError, models::Key, traits::KeyStorage};
use persistent_storage::init::PostgresRepo;

#[async_trait::async_trait]
impl KeyStorage for PostgresRepo {
    async fn insert_key(&self, key: Key) -> Result<(), DatabaseError> {
        let _ = sqlx::query("INSERT INTO keys (key_id) VALUES ($1)")
            .bind(key.key_id)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn get_key(&self, key_id: Uuid) -> Result<Key, DatabaseError> {
        let result: (Uuid,) = sqlx::query_as("SELECT * FROM keys WHERE key_id = $1 LIMIT 1")
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
