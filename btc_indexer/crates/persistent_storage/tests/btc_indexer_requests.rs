mod utils;

mod test_btc_indexer_requests {
    use std::str::FromStr;

    use bitcoin::Txid;
    use config_parser::config::PostgresDbCredentials;
    use global_utils::common_types::{TxIdWrapped, UrlWrapped};
    use persistent_storage::{
        init::PostgresRepo,
        schemas::runes_spark::btc_indexer_work_checkpoint::{BtcIndexerWorkCheckpoint, StatusBtcIndexer, Task},
    };
    use sqlx::types::{Json, chrono::Utc};
    use url::Url;
    use uuid::Uuid;

    use crate::utils::TEST_LOGGER;

    #[sqlx::test]
    async fn test_inserting() -> anyhow::Result<()> {
        dotenv::dotenv()?;
        let _logger_guard = &*TEST_LOGGER;
        let db_entity = PostgresRepo::from_config(PostgresDbCredentials::new()?).await?;
        let pool = db_entity.pool.acquire().await?;
        let value = BtcIndexerWorkCheckpoint {
            uuid: Uuid::new_v4(),
            status: StatusBtcIndexer::Created,
            task: Json::from(Task::TrackTx(TxIdWrapped(Txid::from_str(
                "06b6af9af2e1708335add6c5e99f5ed03e26f3392ce2a3325a3aa7d5588a3983",
            )?))),
            created_at: Utc::now(),
            callback_url: UrlWrapped(Url::from_str("https://example.com/callback")?),
            error: None,
            updated_at: Utc::now(),
        };
        value.insert(pool).await?;
        Ok(())
    }
}
