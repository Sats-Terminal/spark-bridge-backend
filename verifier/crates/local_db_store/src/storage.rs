pub use persistent_storage::error::DbError;
use persistent_storage::{config::*, init::PostgresPool, init::PostgresRepo};

pub struct LocalDbStorage {
    pub postgres_repo: PostgresRepo,
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
