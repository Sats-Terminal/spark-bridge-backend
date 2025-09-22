mod utils;

static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());

use std::{str::FromStr, sync::LazyLock};

use bitcoin::{hashes::Hash, BlockHash};
use bitcoincore_rpc::{bitcoin::Txid, RawTx};
use btc_indexer_internals::{
    api::BtcIndexerApi,
    indexer::{BtcIndexer, IndexerParams, IndexerParamsWithApi},
};
use config_parser::config::{BtcRpcCredentials, ServerConfig};
use global_utils::{
    common_types::get_uuid,
    logger::{init_logger, LoggerGuard},
};
use local_db_store_indexer::PostgresDbCredentials;
use titan_client::TitanApi;
use titan_types::{Transaction, TransactionStatus};
use tracing::debug;

use crate::utils::mock::MockTitanIndexer;

mod mock_testing {
    use super::*;
    use crate::utils::mock::MockTxArbiter;
    use crate::utils::test_notifier::{obtain_random_localhost_socket_addr, spawn_notify_server_track_tx};
    use bitcoin::OutPoint;
    use btc_indexer_api::api::{BtcTxReview, TrackTxRequest};
    use btc_indexer_internals::tx_arbiter::TxArbiterResponse;
    use global_utils::common_types::UrlWrapped;
    use global_utils::config_variant::ConfigVariant;
    use local_db_store_indexer::init::LocalDbStorage;
    use local_db_store_indexer::schemas::tx_tracking_storage::TxToUpdateStatus;
    use persistent_storage::init::PostgresPool;
    use std::env;
    use tracing::info;

    const CONFIG_FILEPATH: &str = "../../../infrastructure/configurations/btc_indexer/dev.toml";

    // Test requires to run Postgres & Docker files (bitcoind + titan)
    #[ignore]
    #[tokio::test]
    async fn init_btc_indexer() -> anyhow::Result<()> {
        dotenv::dotenv()?;
        let _logger_guard = &*TEST_LOGGER;
        let btc_rpc_creds = BtcRpcCredentials::new()?;
        let db_pool = LocalDbStorage::from_config(PostgresDbCredentials::from_envs()?).await?;
        info!("Btc rpc creds: {:?}", btc_rpc_creds);
        let app_config = ServerConfig::init_config(ConfigVariant::OnlyOneFilepath(CONFIG_FILEPATH.to_string()))?;
        let indexer = BtcIndexer::with_api(IndexerParams {
            btc_rpc_creds,
            db_pool,
            btc_indexer_params: app_config.btc_indexer_config,
        })?;
        info!("Blockchain info: {:?}", indexer.get_blockchain_info()?);
        Ok(())
    }

    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../local_db_store/migrations");

    // #[tokio::test]
    // async fn test_retrieving_of_finalized_tx() -> anyhow::Result<()> {

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_retrieving_of_finalized_tx(mut pool: PostgresPool) -> anyhow::Result<()> {
        const MAX_I_INDEX: u64 = 5;

        let tx_id = Txid::from_str("f74516e3b24af90fc2da8251d2c1e3763252b15c7aec3c1a42dde7116138caee")?;
        let uuid = get_uuid();

        dotenv::dotenv()?;
        let _logger_guard = &*TEST_LOGGER;
        let btc_rpc_creds = BtcRpcCredentials::new()?;
        let db_pool = LocalDbStorage::from_config(PostgresDbCredentials::from_envs()?).await?;
        // let db_pool = LocalDbStorage {
        //     postgres_repo: persistent_storage::init::PostgresRepo { pool }.into_shared(),
        // };
        let app_config = ServerConfig::init_config(ConfigVariant::OnlyOneFilepath(CONFIG_FILEPATH.to_string()))?;

        let generate_transaction = |tx_id: &Txid, index: u64| Transaction {
            txid: tx_id.clone(),
            version: 0,
            lock_time: 0,
            input: vec![],
            output: vec![],
            status: if index == MAX_I_INDEX {
                TransactionStatus::confirmed(index, BlockHash::all_zeros())
            } else {
                TransactionStatus::unconfirmed()
            },
            size: 0,
            weight: 0,
        };
        let generate_indexer_mocking_invocations = |indexer: &mut MockTitanIndexer| {
            let mut i = 0;
            indexer.expect_get_transaction().returning(move |tx_id| {
                let utxos = generate_transaction(tx_id, i);
                i += 1;
                let generated_tx = generate_transaction(tx_id, i);
                debug!("[mock expectations1] generated tx: {:?}", &generated_tx);
                Ok(generated_tx)
            });
            indexer.expect_clone().returning(move || {
                let mut cloned_mocked_indexer = MockTitanIndexer::new();
                let mut i = 0;
                cloned_mocked_indexer.expect_get_transaction().returning(move |tx_id| {
                    let utxos = generate_transaction(tx_id, i);
                    i += 1;
                    let generated_tx = generate_transaction(tx_id, i);
                    debug!("[mock expectations2] generated tx: {:?}", &generated_tx);
                    Ok(generated_tx)
                });
                cloned_mocked_indexer
                    .expect_clone()
                    .returning(|| MockTitanIndexer::new());
                cloned_mocked_indexer
            });
        };
        let generate_tx_arbiter_mocking_invocations = |tx_arbiter: &mut MockTxArbiter| {
            tx_arbiter.expect_check_tx().returning(
                |titan_client: MockTitanIndexer, tx_to_check: &Transaction, tx_info: &TxToUpdateStatus| {
                    let review = TxArbiterResponse::ReviewFormed(
                        BtcTxReview::Success,
                        OutPoint {
                            txid: tx_info.tx_id.0,
                            vout: tx_info.v_out,
                        },
                    );
                    debug!("[(tx verifier) mock expectations1], review: {:?}", &review);
                    Ok(review)
                },
            );
            tx_arbiter.expect_clone().returning(move || {
                let mut cloned_tx_arbiter = MockTxArbiter::new();
                cloned_tx_arbiter.expect_check_tx().returning(
                    |titan_client: MockTitanIndexer, tx_to_check: &Transaction, tx_info: &TxToUpdateStatus| {
                        let review = TxArbiterResponse::ReviewFormed(
                            BtcTxReview::Success,
                            OutPoint {
                                txid: tx_info.tx_id.0,
                                vout: tx_info.v_out,
                            },
                        );
                        debug!("[(tx verifier) mock expectations2], review: {:?}", &review);
                        Ok(review)
                    },
                );
                cloned_tx_arbiter.expect_clone().returning(move || {
                    let mut cloned2_tx_arbiter = MockTxArbiter::new();
                    cloned2_tx_arbiter.expect_check_tx().returning(
                        |titan_client: MockTitanIndexer, tx_to_check: &Transaction, tx_info: &TxToUpdateStatus| {
                            let review = TxArbiterResponse::ReviewFormed(
                                BtcTxReview::Success,
                                OutPoint {
                                    txid: tx_info.tx_id.0,
                                    vout: tx_info.v_out,
                                },
                            );
                            debug!("[(tx verifier) mock expectations2], review: {:?}", &review);
                            Ok(review)
                        },
                    );
                    cloned2_tx_arbiter
                });
                cloned_tx_arbiter
            });
        };

        debug!("Initializing mocked indexer");
        let mut mocked_indexer = MockTitanIndexer::new();
        generate_indexer_mocking_invocations(&mut mocked_indexer);
        let mut mocked_tx_arbiter = MockTxArbiter::new();
        generate_tx_arbiter_mocking_invocations(&mut mocked_tx_arbiter);
        debug!("Building BtcIndexer...");
        let indexer = BtcIndexer::new(IndexerParamsWithApi {
            indexer_params: IndexerParams {
                btc_rpc_creds,
                db_pool,
                btc_indexer_params: app_config.btc_indexer_config,
            },
            titan_api_client: mocked_indexer,
            tx_validator: mocked_tx_arbiter,
        })?;
        debug!("Tracking tx changes..");
        let (url_to_listen, oneshot_chan, _notify_server) =
            spawn_notify_server_track_tx(obtain_random_localhost_socket_addr()?).await?;
        debug!("Receiving oneshot event..");
        indexer
            .check_tx_changes(
                uuid,
                &TrackTxRequest {
                    callback_url: UrlWrapped(url_to_listen),
                    btc_address: "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string(),
                    out_point: OutPoint {
                        txid: tx_id,
                        vout: 1234,
                    },
                    amount: 45678,
                },
            )
            .await?;
        let result = oneshot_chan.await?;
        debug!("Event: {result:?}");
        // assert!(compare_address_tx(&result, &generate_transaction(&tx_id, MAX_I_INDEX)));
        Ok(())
    }
}
