use std::fmt::{Debug, Formatter};

use crate::schemas::track_tx_requests_storage::TxRequestsTrackingStorageTrait;
use crate::schemas::tx_tracking_storage::TxTrackingStorageTrait;
use async_trait::async_trait;
use persistent_storage::init::StorageHealthcheck;
use persistent_storage::{
    config::PostgresDbCredentials,
    error::DbError,
    init::{PersistentDbConn, PersistentRepoShared, PersistentRepoTrait, PostgresRepo},
};
use tracing::instrument;

/// Has to be understood as "LocalDb - Indexer"
#[derive(Clone)]
pub struct LocalDbStorage {
    pub postgres_repo: PersistentRepoShared,
}

pub trait IndexerDbBounds:
    PersistentRepoTrait + Clone + TxTrackingStorageTrait + TxRequestsTrackingStorageTrait + StorageHealthcheck + 'static
{
}

#[async_trait]
impl StorageHealthcheck for LocalDbStorage {
    async fn healthcheck(&self) -> Result<(), DbError> {
        self.postgres_repo.healthcheck().await
    }
}

impl IndexerDbBounds for LocalDbStorage {}

impl Debug for LocalDbStorage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Postgres DB")
    }
}

impl LocalDbStorage {
    #[instrument(level = "trace", ret)]
    pub async fn from_config(creds: PostgresDbCredentials) -> Result<Self, DbError> {
        let pool = PostgresRepo::from_config(creds).await?;
        Ok(Self {
            postgres_repo: pool.into_shared(),
        })
    }
}

#[async_trait]
impl PersistentRepoTrait for LocalDbStorage {
    async fn get_conn(&self) -> Result<PersistentDbConn, DbError> {
        self.postgres_repo.get_conn().await
    }
}
