use std::sync::LazyLock;

use global_utils::logger::{LoggerGuard, init_logger};

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());
mod init_tests {
    use persistent_storage::{config::PostgresDbCredentials, init::PostgresRepo};
    use sqlx::Connection;

    use super::*;
    #[tokio::test]
    pub async fn pg_conn_health_check_db_url() -> eyre::Result<()> {
        let _ = dotenvy::dotenv();
        let _ = *TEST_LOGGER;
        let db_entity = PostgresRepo::from_config(PostgresDbCredentials::from_db_url()?).await?;
        let mut conn = db_entity.pool.acquire().await?;
        assert_eq!(conn.ping().await?, ());
        Ok(())
    }

    #[tokio::test]
    pub async fn pg_conn_health_check_envs() -> eyre::Result<()> {
        let _ = dotenvy::dotenv();
        let _ = *TEST_LOGGER;
        let db_entity = PostgresRepo::from_config(PostgresDbCredentials::from_envs()?).await?;
        let mut conn = db_entity.pool.acquire().await?;
        assert_eq!(conn.ping().await?, ());
        Ok(())
    }
}
