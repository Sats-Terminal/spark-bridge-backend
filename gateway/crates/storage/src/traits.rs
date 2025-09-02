use uuid::Uuid;

use crate::{
    errors::DatabaseError,
    models::{Key, Request},
};

#[async_trait::async_trait]
pub trait RequestStorage {
    async fn insert_request(&self, request: Request) -> Result<(), DatabaseError>;

    async fn get_request(&self, request_id: Uuid) -> Result<Request, DatabaseError>;
}

#[async_trait::async_trait]
pub trait KeyStorage {
    async fn insert_key(&self, key: Key) -> Result<(), DatabaseError>;

    async fn get_key(&self, key_id: Uuid) -> Result<Key, DatabaseError>;
}
