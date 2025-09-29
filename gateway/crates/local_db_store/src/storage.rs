use std::sync::Arc;
use bitcoin::Network;
use global_utils::config_path::ConfigPath;
pub use persistent_storage::error::DbError;
pub(crate) use persistent_storage::{
    config::*,
    init::{PostgresPool, PostgresRepo},
};
use gateway_config_parser::config::ServerConfig;

#[derive(Clone, Debug)]
pub struct LocalDbStorage {
    pub postgres_repo: PostgresRepo,
    pub network: Network,
}

impl LocalDbStorage {
    pub async fn new(database_url: String, network: Network) -> Result<Self, DbError> {
        let postgres_repo = PostgresRepo::from_config(PostgresDbCredentials { url: database_url }).await?;
        Ok(Self {
            postgres_repo,
            network
        })
    }

    pub async fn get_conn(&self) -> Result<PostgresPool, DbError> {
        Ok(self.postgres_repo.pool.clone())
    }
}

pub async fn make_repo_with_config(db: sqlx::PgPool) -> Arc<LocalDbStorage> {
    let config_path = ConfigPath::from_env().unwrap();
    let server_config = ServerConfig::init_config(config_path.path);

    Arc::new(LocalDbStorage {
        postgres_repo: PostgresRepo { pool: db },
        network: server_config.network.network
    })
}
