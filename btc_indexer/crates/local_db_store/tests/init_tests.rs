mod utils;

mod init_tests {
    use persistent_storage::{config::PostgresDbCredentials, init::PostgresRepo};
    use sqlx::Connection;

    use crate::utils::TEST_LOGGER;

    #[tokio::test]
    pub async fn pg_conn_health_check() -> eyre::Result<()> {
        let _ = dotenvy::dotenv();
        let _ = *TEST_LOGGER;
        let db_entity = PostgresRepo::from_config(PostgresDbCredentials::from_db_url()?).await?;
        let mut conn = db_entity.pool.acquire().await?;
        assert_eq!(conn.ping().await?, ());
        Ok(())
    }
}
