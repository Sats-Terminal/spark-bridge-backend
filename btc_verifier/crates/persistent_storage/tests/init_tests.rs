#[cfg(test)]
mod init_tests {
    use std::sync::LazyLock;

    use config_parser::config::PostgresDbCredentials;
    use global_utils::logger::{LoggerGuard, init_logger};
    use persistent_storage::init::{PostgresPool, PostgresRepo};
    use sqlx::Connection;
    use tracing::info;

    static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());

    #[tokio::test]
    pub async fn test_invocation() -> anyhow::Result<()> {
        let _ = *TEST_LOGGER;
        let _ = dotenv::dotenv();
        let db_entity = PostgresRepo::from_config(PostgresDbCredentials::new()?).await?;
        let mut conn = db_entity.pool.acquire().await?;
        assert_eq!(conn.ping().await?, ());
        Ok(())
    }
}
