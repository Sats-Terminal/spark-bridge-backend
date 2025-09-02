use crate::errors::DatabaseError;
use persistent_storage::init::PostgresRepo;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Request {
    pub request_id: Uuid,
    pub key_id: Uuid,
}

#[async_trait::async_trait]
pub trait RequestStorage {
    async fn insert_request(&self, request: Request) -> Result<(), DatabaseError>;

    async fn get_request(&self, request_id: Uuid) -> Result<Request, DatabaseError>;
}

#[async_trait::async_trait]
impl RequestStorage for PostgresRepo {
    async fn insert_request(&self, request: Request) -> Result<(), DatabaseError> {
        let _ = sqlx::query("INSERT INTO gateway.requests (request_id, key_id) VALUES ($1, $2)")
            .bind(request.request_id)
            .bind(request.key_id)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn get_request(&self, request_id: Uuid) -> Result<Request, DatabaseError> {
        let result: (Uuid, Uuid) = sqlx::query_as("SELECT * FROM gateway.requests WHERE request_id = $1 LIMIT 1")
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
