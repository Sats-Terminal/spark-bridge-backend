use crate::{models::Key, traits::KeyStorage};
use persistent_storage::init::PostgresRepo;
use crate::errors::DatabaseError;
use sqlx;
use uuid::Uuid;

impl KeyStorage for PostgresRepo {
    async fn get_key(&self, key_id: &Uuid) -> Result<Key, DatabaseError> {
        let key = sqlx::query_as!(
            Key,
            "SELECT key_id, metadata FROM keys WHERE key_id = $1",
            key_id
        )
        .fetch_one(&self.pool)
        .await.map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(key)
    }

    async fn create_key(&self, key: &Key) -> Result<(), DatabaseError> {
        let _ = sqlx::query!(
            "INSERT INTO keys (key_id, metadata) VALUES ($1, $2) RETURNING key_id, metadata",
            key.key_id,
            key.metadata
        )
        .fetch_one(&self.pool)
        .await.map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
