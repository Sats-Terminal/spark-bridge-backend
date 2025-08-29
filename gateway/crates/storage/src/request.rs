use uuid::Uuid;

use crate::{Storage, errors::DatabaseError, models::Request, traits::RequestStorage};

impl RequestStorage for Storage {
    async fn insert_request(&self, request: Request) -> Result<(), DatabaseError> {
        let pool = self.pool.lock().unwrap();
        let _ = sqlx::query!("INSERT INTO requests (request_id) VALUES ($1)", request.request_id)
            .execute(&*pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn get_request(&self, request_id: Uuid) -> Result<Request, DatabaseError> {
        let pool = self.pool.lock().unwrap();
        let result = sqlx::query!("SELECT * FROM requests WHERE request_id = $1 LIMIT 1", request_id)
            .fetch_one(&*pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => DatabaseError::NotFound(e.to_string()),
                _ => DatabaseError::BadRequest(e.to_string()),
            })?;

        Ok(Request {
            request_id: result.request_id,
            key_id: result.key_id,
        })
    }
}
