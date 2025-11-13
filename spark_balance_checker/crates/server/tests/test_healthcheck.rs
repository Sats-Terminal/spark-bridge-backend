mod utils;
mod test_healthcheck {
    use axum_test::http::StatusCode;
    use spark_balance_checker_server::init::HEALTHCHECK_ENDPOINT;
    use tracing::instrument;

    use crate::utils::{TEST_LOGGER, init_mocked_test_server};

    #[instrument]
    #[tokio::test]
    async fn test_invocation_tx_tracking() -> eyre::Result<()> {
        let _logger_guard = &*TEST_LOGGER;
        let test_server = init_mocked_test_server().await?;
        let response = test_server.get(HEALTHCHECK_ENDPOINT).await;
        assert_eq!(response.status_code(), StatusCode::OK);
        Ok(())
    }
}
