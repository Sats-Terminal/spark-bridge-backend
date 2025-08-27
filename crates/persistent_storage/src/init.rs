use std::sync::Arc;

use async_trait::async_trait;
use sqlx::{PgPool, Pool, Postgres, pool::PoolConnection};
use tracing::{instrument, trace};

use crate::{
    config::{PostgresDbCredentials, PostgresDbTestingCredentials},
    error::{DbError, DbInitError},
};

pub type PostgresPool = Pool<Postgres>;
pub type PersistentDbConn = PoolConnection<Postgres>;
pub type PersistentRepoShared = Arc<Box<dyn PersistentRepoTrait>>;

#[derive(Debug, Clone)]
pub struct PostgresRepo {
    pub pool: PostgresPool,
}

#[derive(Debug, Clone)]
pub struct PostgresRepoTesting {
    pub pool: PostgresPool,
}

/// Trait for implementing Persistent storage that'd use Postgres
#[async_trait]
pub trait PersistentRepoTrait: Send + Sync {
    async fn get_conn(&self) -> crate::error::Result<PersistentDbConn>;
}

impl PostgresRepoTesting {
    #[instrument(level = "trace", ret)]
    pub async fn from_config(creds: PostgresDbTestingCredentials) -> Result<Self, DbInitError> {
        trace!("Creating Redis connection pool with creds: {:?}", creds);
        let pool = PgPool::connect(&creds.url)
            .await
            .map_err(|x| DbInitError::FailedToEstablishDbConn(x, creds.url.clone()))?;
        trace!(db_url = creds.url, "Creating [testing] Postgres pool with config");
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn into_shared(self) -> PersistentRepoShared {
        Arc::new(Box::new(self))
    }

    pub async fn ping(conn: &mut PersistentDbConn) -> Result<(), DbError> {
        db_helpers::ping(conn).await
    }
}

#[async_trait]
impl PersistentRepoTrait for PostgresRepoTesting {
    async fn get_conn(&self) -> crate::error::Result<PersistentDbConn> {
        Ok(self.pool.acquire().await?)
    }
}

impl PostgresRepo {
    #[instrument(level = "trace", ret)]
    pub async fn from_config(creds: PostgresDbCredentials) -> Result<Self, DbInitError> {
        trace!("Creating Redis connection pool with creds: {:?}", creds);
        let pool = PgPool::connect(&creds.url)
            .await
            .map_err(|x| DbInitError::FailedToEstablishDbConn(x, creds.url.clone()))?;
        trace!(db_url = creds.url, "Creating Postgres pool with config");
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn into_shared(self) -> PersistentRepoShared {
        Arc::new(Box::new(self))
    }

    pub async fn ping(conn: &mut PersistentDbConn) -> Result<(), DbError> {
        db_helpers::ping(conn).await
    }
}

#[async_trait]
impl PersistentRepoTrait for PostgresRepo {
    async fn get_conn(&self) -> crate::error::Result<PersistentDbConn> {
        Ok(self.pool.acquire().await?)
    }
}

pub mod db_helpers {
    use sqlx::Connection;

    use crate::{error::DbError, init::PersistentDbConn};

    #[inline]
    pub async fn ping(conn: &mut PersistentDbConn) -> Result<(), DbError> {
        Ok(conn.ping().await?)
    }
}
