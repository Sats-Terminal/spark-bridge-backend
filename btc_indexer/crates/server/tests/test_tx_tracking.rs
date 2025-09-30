mod utils;

mod mocked_tx_tracking {
    use std::str::FromStr;

    use crate::utils::comparing_utils::btc_indexer_callback_response_eq;
    use crate::utils::init::MIGRATOR;
    use crate::utils::mock::generate_mock_tx_arbiter;
    use crate::utils::{
        init::{TEST_LOGGER, obtain_random_localhost_socket_addr},
        mock::{
            create_app_mocked, generate_mock_titan_indexer_tx_tracking, generate_mock_titan_indexer_wallet_tracking,
        },
        test_notifier::spawn_notify_server_track_tx,
    };
    use axum_test::TestServer;
    use bitcoin::{OutPoint, Txid};
    use btc_indexer_api::api::{BtcIndexerCallbackResponse, BtcTxReview, ResponseMeta, TrackTxRequest};
    use btc_indexer_internals::indexer::{BtcIndexer, IndexerParams, IndexerParamsWithApi};
    use config_parser::config::ServerConfig;
    use global_utils::common_types::{TxIdWrapped, UrlWrapped};
    use ordinals::RuneId;
    use persistent_storage::init::PostgresPool;
    use tracing::{info, instrument};

    pub async fn init_mocked_tx_tracking_test_server(pool: PostgresPool) -> anyhow::Result<TestServer> {
        Ok(crate::utils::mock::init_mocked_test_server(
            || generate_mock_titan_indexer_tx_tracking(),
            || generate_mock_tx_arbiter(),
            pool,
        )
        .await?)
    }

    #[instrument]
    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_invocation_tx_tracking(pool: PostgresPool) -> anyhow::Result<()> {
        dotenvy::dotenv()?;
        let _logger_guard = &*TEST_LOGGER;
        let test_server = init_mocked_tx_tracking_test_server(pool).await?;
        let (url_to_listen, oneshot_chan, _notify_server) =
            spawn_notify_server_track_tx(obtain_random_localhost_socket_addr()?).await?;
        let out_point = OutPoint {
            txid: Txid::from_str("fb0c9ab881331ec7acdd85d79e3197dcaf3f95055af1703aeee87e0d853e81ec")?,
            vout: 1234,
        };
        let response = test_server
            .post("/track_tx")
            .json(&TrackTxRequest {
                callback_url: UrlWrapped(url_to_listen),
                btc_address: "".to_string(),
                rune_amount: 5678,
                out_point,
                rune_id: RuneId::from_str("1:0")?,
            })
            .await;
        info!("First subscription [track_tx] response: {:?}", response);

        let result = oneshot_chan.await?;
        info!("Callback response: {:?}", result);
        assert!(btc_indexer_callback_response_eq(
            &result,
            &BtcIndexerCallbackResponse::Ok {
                meta: ResponseMeta {
                    outpoint: out_point,
                    status: BtcTxReview::Success,
                    sats_fee_amount: 0,
                }
            }
        ));
        Ok(())
    }
}
