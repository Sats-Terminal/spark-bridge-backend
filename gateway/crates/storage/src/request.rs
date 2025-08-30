use uuid::Uuid;
use persistent_storage::init::PostgresRepo;
use crate::{errors::DatabaseError, models::Request, traits::RequestStorage};

impl RequestStorage for PostgresRepo {
    async fn insert_request(&self, request: Request) -> Result<(), DatabaseError> {
        let _ = sqlx::query("INSERT INTO requests (request_id, key_id) VALUES ($1, $2)")
            .bind(request.request_id)
            .bind(request.key_id)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn get_request(&self, request_id: Uuid) -> Result<Request, DatabaseError> {
        let result: (Uuid, Uuid) = sqlx::query_as("SELECT * FROM requests WHERE request_id = $1 LIMIT 1")
            .bind(request_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => DatabaseError::NotFound(e.to_string()),
                _ => DatabaseError::BadRequest(e.to_string()),
            })?;

        Ok(Request {
            request_id: result.0,
            key_id: result.1,
        })
    }
}
