use persistent_storage::init::PostgresRepo;
use bitcoin::Network;
use btc_indexer_config::DatabaseConfig;
use persistent_storage::error::DbError;
use persistent_storage::config::PostgresDbCredentials;


#[derive(Clone, Debug)]
pub struct LocalDbStorage {
    pub postgres_repo: PostgresRepo,
    pub network: Network,
}

impl LocalDbStorage {
    pub async fn new(config: DatabaseConfig, network: Network) -> Result<Self, DbError> {
        let postgres_repo = PostgresRepo::from_config(PostgresDbCredentials { url: config.url }).await?;
        Ok(Self { postgres_repo, network })
    }
}
