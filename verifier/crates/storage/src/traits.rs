use persistent_storage::init::{PostgresRepo, PersistentRepoTrait};
use crate::{models::Key, errors::DatabaseError};
use uuid::Uuid;

pub trait KeyStorage {
    async fn get_key(&self, key_id: &Uuid) -> Result<Key, DatabaseError>;
    async fn create_key(&self, key: &Key) -> Result<(), DatabaseError>;
}