use persistent_storage::init::{PostgresRepo, PersistentRepoTrait};
use crate::{models::Key, errors::DatabaseError};

pub trait KeyStorage {
    async fn get_key(&self, key_id: &str) -> Result<Key, DatabaseError>;
    async fn create_key(&self, key: &Key) -> Result<(), DatabaseError>;
}