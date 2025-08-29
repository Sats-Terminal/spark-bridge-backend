use crate::models::Request;
use crate::errors::DatabaseError;
use uuid::Uuid;

pub trait RequestStorage {
    async fn insert_request(&self, request: Request) -> Result<(), DatabaseError>;

    async fn get_request(&self, request_id: Uuid) -> Result<Request, DatabaseError>;
}