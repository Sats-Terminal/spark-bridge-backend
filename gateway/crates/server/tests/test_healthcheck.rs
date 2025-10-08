mod utils;
mod test_healthcheck {
    use crate::utils::common::MIGRATOR;
    use crate::utils::common::TEST_LOGGER;
    use crate::utils::healthcheck_mock::init_mocked_test_server;
    use axum_test::http::StatusCode;
    use gateway_server::init::GatewayApi;
    use persistent_storage::init::PostgresPool;
    use tracing::instrument;

    #[instrument(ret)]
    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_invocation_tx_tracking(pool: PostgresPool) -> eyre::Result<()> {
        let _logger_guard = &*TEST_LOGGER;
        let test_server = init_mocked_test_server(pool).await?;
        let response = test_server.get(GatewayApi::HEALTHCHECK_ENDPOINT).await;
        assert_eq!(response.status_code(), StatusCode::OK);
        Ok(())
    }
}
