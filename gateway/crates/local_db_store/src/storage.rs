use bitcoin::Network;
use frost::aggregator::FrostAggregator;
use gateway_config_parser::config::ServerConfig;
use global_utils::config_path::ConfigPath;
pub use persistent_storage::error::DbError;
pub(crate) use persistent_storage::{
    config::*,
    init::{PostgresPool, PostgresRepo},
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct LocalDbStorage {
    pub postgres_repo: PostgresRepo,
    pub btc_network: Network,
}

impl LocalDbStorage {
    pub async fn new(database_url: String, btc_network: Network) -> Result<Self, DbError> {
        let postgres_repo = PostgresRepo::from_config(PostgresDbCredentials { url: database_url }).await?;
        Ok(Self {
            postgres_repo,
            btc_network,
        })
    }

    pub async fn get_conn(&self) -> Result<PostgresPool, DbError> {
        Ok(self.postgres_repo.pool.clone())
    }

    pub fn into_shared(self) -> Arc<Self> {
        Arc::new(self)
    }
}

pub async fn make_repo_with_config(db: sqlx::PgPool) -> Arc<LocalDbStorage> {
    let config_path = ConfigPath::from_env().unwrap();
    let server_config = ServerConfig::init_config(config_path.path);

    Arc::new(LocalDbStorage {
        postgres_repo: PostgresRepo { pool: db },
        btc_network: server_config.network.network,
    })
}
