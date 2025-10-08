mod utils;

mod mocked_tx_tracking {
    use std::str::FromStr;

    use crate::utils::comparing_utils::btc_indexer_meta_eq;
    use crate::utils::init::MIGRATOR;
    use crate::utils::mock::{generate_mock_titan_indexer_tx_tracking_empty, generate_mock_tx_arbiter};
    use crate::utils::{
        init::{TEST_LOGGER, obtain_random_localhost_socket_addr},
        mock::generate_mock_titan_indexer_tx_tracking_custom,
        test_notifier::spawn_notify_server_track_tx,
    };
    use axum_test::TestServer;
    use bitcoin::hashes::Hash;
    use bitcoin::{BlockHash, OutPoint, Txid};
    use btc_indexer_api::api::{BtcIndexerCallbackResponse, BtcTxReview, TrackTxRequest, TxRejectReason};

    use global_utils::common_types::UrlWrapped;
    use ordinals::RuneId;
    use persistent_storage::init::PostgresPool;
    use titan_types::{SpentStatus, Transaction, TransactionStatus, TxOut};
    use tracing::{info, instrument};

    #[instrument(skip(pool))]
    pub async fn init_mocked_tx_tracking_test_server_nonempty(pool: PostgresPool) -> eyre::Result<TestServer> {
        Ok(crate::utils::mock::init_mocked_test_server(
            || {
                generate_mock_titan_indexer_tx_tracking_custom(Transaction {
                    txid: Txid::from_str("baed3ef0c9812fe2b13af7c5228c7d6fe5d74b58ed117bf1bb90f905c63144e7").unwrap(),
                    version: 0,
                    lock_time: 0,
                    input: vec![],
                    output: vec![TxOut {
                        runes: vec![],
                        risky_runes: vec![],
                        value: 1999,
                        spent: SpentStatus::Unspent,
                        script_pubkey: Default::default(),
                    }],
                    status: TransactionStatus::confirmed(100, BlockHash::all_zeros()),
                    size: 0,
                    weight: 0,
                })
            },
            || generate_mock_tx_arbiter(),
            pool,
        )
        .await?)
    }

    #[instrument(skip(pool))]
    pub async fn init_mocked_tx_tracking_test_server_empty(pool: PostgresPool) -> eyre::Result<TestServer> {
        Ok(crate::utils::mock::init_mocked_test_server(
            || generate_mock_titan_indexer_tx_tracking_empty(),
            || generate_mock_tx_arbiter(),
            pool,
        )
        .await?)
    }

    #[instrument]
    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_invocation_tx_tracking_failure(pool: PostgresPool) -> eyre::Result<()> {
        dotenvy::dotenv();
        let _logger_guard = &*TEST_LOGGER;
        let test_server = init_mocked_tx_tracking_test_server_empty(pool).await?;
        let (url_to_listen, oneshot_chan, _notify_server) =
            spawn_notify_server_track_tx(obtain_random_localhost_socket_addr()?).await?;
        let out_point = OutPoint {
            txid: Txid::from_str("fb0c9ab881331ec7acdd85d79e3197dcaf3f95055af1703aeee87e0d853e81ec")?,
            vout: 0,
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
        assert!(btc_indexer_meta_eq(
            &result,
            &BtcIndexerCallbackResponse {
                outpoint: out_point,
                status: BtcTxReview::Failure {
                    reason: TxRejectReason::NoExpectedVOutInOutputs { got: 0, expected: 1 },
                },
                sats_amount: 0,
            }
        ));
        Ok(())
    }

    #[instrument]
    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_invocation_tx_success(pool: PostgresPool) -> eyre::Result<()> {
        dotenvy::dotenv();
        let _logger_guard = &*TEST_LOGGER;
        let test_server = init_mocked_tx_tracking_test_server_nonempty(pool).await?;
        let (url_to_listen, oneshot_chan, _notify_server) =
            spawn_notify_server_track_tx(obtain_random_localhost_socket_addr()?).await?;
        let out_point = OutPoint {
            txid: Txid::from_str("fb0c9ab881331ec7acdd85d79e3197dcaf3f95055af1703aeee87e0d853e81ec")?,
            vout: 0,
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
        assert!(btc_indexer_meta_eq(
            &result,
            &BtcIndexerCallbackResponse {
                outpoint: out_point,
                status: BtcTxReview::Success,
                sats_amount: 0,
            }
        ));
        Ok(())
    }
}
