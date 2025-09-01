use std::fmt::{Debug, Formatter};

use async_trait::async_trait;
use persistent_storage::{
    config::PostgresDbCredentials,
    error::{DbGetConnError, DbInitError},
    init::{PersistentDbConn, PersistentRepoShared, PersistentRepoTrait, PostgresRepo},
};
use tracing::{info, instrument};

#[derive(Clone)]
pub struct LocalDbStorage {
    pub postgres_repo: PersistentRepoShared,
}

impl Debug for LocalDbStorage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Postgres DB")
    }
}

impl LocalDbStorage {
    #[instrument(level = "trace", ret)]
    pub async fn from_config(creds: PostgresDbCredentials) -> Result<Self, DbInitError> {
        let mut pool = PostgresRepo::from_config(creds).await?;
        info!("initializing local db");
        sqlx::migrate!().run(&pool.pool).await?;
        Ok(Self {
            postgres_repo: pool.into_shared(),
        })
    }
}

#[async_trait]
impl PersistentRepoTrait for LocalDbStorage {
    async fn get_conn(&self) -> Result<PersistentDbConn, DbGetConnError> {
        self.postgres_repo.get_conn().await
    }
}
