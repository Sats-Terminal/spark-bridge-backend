use crate::errors::DatabaseError;
use crate::{models::Key, traits::KeyStorage};
use persistent_storage::init::PostgresRepo;
use sqlx::{self, Row};
use uuid::Uuid;

impl KeyStorage for PostgresRepo {
    async fn get_key(&self, key_id: &Uuid) -> Result<Key, DatabaseError> {
        let result: (Uuid, String) = sqlx::query_as("SELECT key_id, metadata FROM keys WHERE key_id = $1")
            .bind(key_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(Key {
            key_id: result.0,
            metadata: result.1,
        })
    }

    async fn create_key(&self, key: &Key) -> Result<(), DatabaseError> {
        let _ = sqlx::query("INSERT INTO keys (key_id, metadata) VALUES ($1, $2) RETURNING key_id, metadata")
            .bind(key.key_id)
            .bind(key.metadata.clone())
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
