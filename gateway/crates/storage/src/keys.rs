use sqlx;
use uuid::Uuid;

use crate::{Storage, errors::DatabaseError, models::Key, traits::KeyStorage};

impl KeyStorage for Storage {
    async fn insert_key(&self, key: Key) -> Result<(), DatabaseError> {
        let pool = self.pool.lock().unwrap();
        let _ = sqlx::query!("INSERT INTO keys (key_id) VALUES ($1)", key.key_id)
            .execute(&*pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn get_key(&self, key_id: Uuid) -> Result<Key, DatabaseError> {
        let pool = self.pool.lock().unwrap();
        let result = sqlx::query!("SELECT * FROM keys WHERE key_id = $1 LIMIT 1", key_id)
            .fetch_one(&*pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => DatabaseError::NotFound(e.to_string()),
                _ => DatabaseError::BadRequest(e.to_string()),
            })?;

        Ok(Key { key_id: result.key_id })
    }
}
