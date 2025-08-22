use std::str::FromStr;

use axum_test::TestServer;
use bitcoin::Txid;
use btc_indexer_server::{
    common::{TxIdWrapped, UrlWrapped},
    routes::track_tx::TrackTxRequest,
};
use tracing::{info, instrument};

use crate::utils::{
    init::{TEST_LOGGER, init_test_server, obtain_random_localhost_socket_addr},
    test_notifier::spawn_notify_server_track_tx,
};

mod utils;

mod mocked_tx_tracking {
    use btc_indexer_internals::indexer::{BtcIndexer, IndexerParams, IndexerParamsWithApi};
    use config_parser::config::{BtcRpcCredentials, ConfigVariant, PostgresDbCredentials, ServerConfig};
    use persistent_storage::init::PostgresRepo;

    use super::*;
    use crate::utils::mock::{create_app_mocked, generate_mock_titan_indexer_tx_tracking};

    #[instrument(level = "debug", ret)]
    pub async fn init_mocked_test_server() -> anyhow::Result<TestServer> {
        let _logger_guard = &*TEST_LOGGER;
        let (btc_creds, postgres_creds, config_variant) = (
            BtcRpcCredentials::new()?,
            PostgresDbCredentials::new()?,
            ConfigVariant::Local,
        );
        let app_config = ServerConfig::init_config(config_variant)?;
        let db_pool = PostgresRepo::from_config(postgres_creds).await?.into_shared();
        let mocked_titan_indexer = generate_mock_titan_indexer_tx_tracking();
        let btc_indexer = BtcIndexer::new(IndexerParamsWithApi {
            indexer_params: IndexerParams {
                btc_rpc_creds: btc_creds,
                db_pool: db_pool.clone(),
                btc_indexer_params: app_config.btc_indexer_config,
            },
            titan_api_client: mocked_titan_indexer,
        })?;
        let app = create_app_mocked(db_pool, btc_indexer).await;
        let test_server = TestServer::builder().http_transport().build(app.into_make_service())?;
        tracing::info!("Serving local axum test server on {:?}", test_server.server_address());
        Ok(test_server)
    }

    #[tokio::test]
    #[instrument]
    async fn test_invocation_tx_tracking() -> anyhow::Result<()> {
        dotenv::dotenv()?;
        let _logger_guard = &*TEST_LOGGER;
        let test_server = init_mocked_test_server().await?;
        let (url_to_listen, oneshot_chan, _notify_server) =
            spawn_notify_server_track_tx(obtain_random_localhost_socket_addr()?).await?;
        let response = test_server
            .post("/track_tx")
            .json(&TrackTxRequest {
                tx_id: TxIdWrapped(Txid::from_str(
                    "fb0c9ab881331ec7acdd85d79e3197dcaf3f95055af1703aeee87e0d853e81ec",
                )?),
                callback_url: UrlWrapped(url_to_listen),
            })
            .await;
        info!("First subscription [track_tx] response: {:?}", response);

        let result = oneshot_chan.await?;
        info!("ApiResponseOwned result: {:?}", result);
        Ok(())
    }
}
