mod utils;
mod mocked_healthcheck {
    use crate::utils::init::MIGRATOR;
    use crate::utils::mock::{generate_mock_titan_indexer_tx_tracking_empty, generate_mock_tx_arbiter};
    use crate::utils::{
        init::{TEST_LOGGER, obtain_random_localhost_socket_addr},
        mock::generate_mock_titan_indexer_tx_tracking_custom,
        test_notifier::spawn_notify_server_track_tx,
    };
    use axum_test::TestServer;
    use axum_test::http::StatusCode;
    use btc_indexer_api::api::BtcIndexerApi;
    use persistent_storage::init::PostgresPool;
    use tracing::instrument;

    pub async fn init_mocked_tx_tracking_test_server(pool: PostgresPool) -> eyre::Result<TestServer> {
        Ok(crate::utils::mock::init_mocked_test_server(
            || generate_mock_titan_indexer_tx_tracking_empty(),
            || generate_mock_tx_arbiter(),
            pool,
        )
        .await?)
    }

    #[instrument]
    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_invocation_tx_tracking(pool: PostgresPool) -> eyre::Result<()> {
        dotenvy::dotenv();
        let _logger_guard = &*TEST_LOGGER;
        let test_server = init_mocked_tx_tracking_test_server(pool).await?;
        let (_url_to_listen, _oneshot_chan, _notify_server) =
            spawn_notify_server_track_tx(obtain_random_localhost_socket_addr()?).await?;

        let response = test_server.get(BtcIndexerApi::HEALTHCHECK_ENDPOINT).await;
        assert_eq!(response.status_code(), StatusCode::OK);
        Ok(())
    }
}
