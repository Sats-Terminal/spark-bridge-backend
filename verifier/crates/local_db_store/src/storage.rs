use async_trait::async_trait;
pub use persistent_storage::error::DbError;
use persistent_storage::init::StorageHealthcheck;
use persistent_storage::{config::*, init::PostgresPool, init::PostgresRepo};

#[derive(Clone, Debug)]
pub struct LocalDbStorage {
    pub postgres_repo: PostgresRepo,
}

#[async_trait]
impl StorageHealthcheck for LocalDbStorage {
    async fn healthcheck(&self) -> Result<(), DbError> {
        self.postgres_repo.healthcheck().await
    }
}

impl LocalDbStorage {
    pub async fn new(database_url: String) -> Result<Self, DbError> {
        let postgres_repo = PostgresRepo::from_config(PostgresDbCredentials { url: database_url }).await?;
        Ok(Self { postgres_repo })
    }

    pub async fn get_conn(&self) -> Result<PostgresPool, DbError> {
        Ok(self.postgres_repo.pool.clone())
    }
}
