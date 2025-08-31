use crate::{errors::DatabaseError, models::Key};
use persistent_storage::init::{PersistentRepoTrait, PostgresRepo};
use uuid::Uuid;

pub trait KeyStorage {
    async fn get_key(&self, key_id: &Uuid) -> Result<Key, DatabaseError>;
    async fn create_key(&self, key: &Key) -> Result<(), DatabaseError>;
}
