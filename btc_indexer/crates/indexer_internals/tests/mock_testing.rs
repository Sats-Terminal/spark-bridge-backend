mod utils;

use std::{collections::HashMap, str::FromStr};

use bitcoin::hashes::Hash;
use bitcoin::{BlockHash, Transaction as CoreTransaction, absolute::LockTime, transaction::Version};
use bitcoin_rpc_client::BitcoinRpcClient;
use bitcoincore_rpc::bitcoin::Txid;
use bitcoincore_rpc_json::{GetBlockchainInfoResult, StringOrStringArray};
use btc_indexer_internals::{
    api::BtcIndexerApi,
    indexer::{BtcIndexer, IndexerParams, IndexerParamsWithApi},
};
use config_parser::config::{BtcRpcCredentials, ServerConfig};
use global_utils::common_types::get_uuid;
use std::collections::HashMap;
use titan_types::{Transaction, TransactionStatus};
use tracing::debug;

use crate::utils::mock::MockTitanIndexer;

mod mock_testing {
    use super::*;
    use crate::utils::common::TEST_LOGGER;
    use crate::utils::comparing_utils::btc_indexer_response_meta_eq;
    use crate::utils::mock::MockTxArbiter;
    use crate::utils::test_notifier::{obtain_random_localhost_socket_addr, spawn_notify_server_track_tx};
    use bitcoin::OutPoint;
    use btc_indexer_api::api::{Amount, BtcTxReview, ResponseMeta, TrackTxRequest};
    use btc_indexer_internals::tx_arbiter::TxArbiterResponse;
    use config_parser::config::TitanConfig;
    use global_utils::common_types::UrlWrapped;
    use global_utils::config_variant::ConfigVariant;
    use local_db_store_indexer::init::LocalDbStorage;
    use local_db_store_indexer::schemas::tx_tracking_storage::TxToUpdateStatus;
    use persistent_storage::init::PostgresPool;
    use std::sync::Arc;
    use url::Url;

    const CONFIG_FILEPATH: &str = "../../../infrastructure/configurations/btc_indexer/dev.toml";
    const TITAN_URL: &str = "http://localhost:3030";

    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../local_db_store/migrations");

    const VOUT_FOR_OUT_POINT: u32 = 1234;
    const RUNE_AMOUNT: Amount = 45678;

    struct StubBitcoinRpcClient;

    impl BitcoinRpcClient for StubBitcoinRpcClient {
        fn get_transaction(&self, _txid: &Txid) -> bitcoin_rpc_client::Result<CoreTransaction> {
            Ok(CoreTransaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: Vec::new(),
                output: Vec::new(),
            })
        }

        fn get_blockchain_info(&self) -> bitcoin_rpc_client::Result<GetBlockchainInfoResult> {
            Ok(GetBlockchainInfoResult {
                chain: bitcoin::Network::Regtest,
                blocks: 0,
                headers: 0,
                best_block_hash: BlockHash::all_zeros(),
                difficulty: 0.0,
                median_time: 0,
                verification_progress: 0.0,
                initial_block_download: false,
                chain_work: Vec::new(),
                size_on_disk: 0,
                pruned: false,
                prune_height: None,
                automatic_pruning: None,
                prune_target_size: None,
                softforks: HashMap::new(),
                warnings: StringOrStringArray::String(String::new()),
            })
        }

        fn send_raw_transaction(&self, _tx_hex: &str) -> bitcoin_rpc_client::Result<Txid> {
            Ok(Txid::all_zeros())
        }
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_retrieving_of_finalized_tx(pool: PostgresPool) -> anyhow::Result<()> {
        dotenvy::dotenv();
        let _logger_guard = &*TEST_LOGGER;

        let tx_id = Txid::from_str("f74516e3b24af90fc2da8251d2c1e3763252b15c7aec3c1a42dde7116138caee")?;
        let uuid = get_uuid();

        let btc_rpc_creds = BtcRpcCredentials::new()?;
        let db_pool = LocalDbStorage {
            postgres_repo: persistent_storage::init::PostgresRepo { pool }.into_shared(),
        };
        let app_config = ServerConfig::init_config(ConfigVariant::OnlyOneFilepath(CONFIG_FILEPATH.to_string()))?;

        let indexer = generate_mocking_expectations(btc_rpc_creds, db_pool, app_config)?;
        debug!("Tracking tx changes..");
        let (url_to_listen, oneshot_chan, _notify_server) =
            spawn_notify_server_track_tx(obtain_random_localhost_socket_addr()?).await?;
        debug!("Receiving oneshot event..");
        let out_point = OutPoint {
            txid: tx_id,
            vout: VOUT_FOR_OUT_POINT,
        };
        indexer
            .check_tx_changes(
                uuid,
                &TrackTxRequest {
                    callback_url: UrlWrapped(url_to_listen),
                    btc_address: "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string(),
                    out_point,
                    rune_id: ordinals::RuneId::from_str("840000:142")?,
                    rune_amount: RUNE_AMOUNT,
                },
            )
            .await?;
        let result = oneshot_chan.await?;
        assert!(btc_indexer_response_meta_eq(
            &result,
            &ResponseMeta {
                outpoint: out_point,
                status: BtcTxReview::Success,
                sats_fee_amount: 0,
            }
        ));
        Ok(())
    }

    fn generate_mocking_expectations(
        btc_rpc_creds: BtcRpcCredentials,
        db_pool: LocalDbStorage,
        app_config: ServerConfig,
    ) -> anyhow::Result<BtcIndexer<MockTitanIndexer, LocalDbStorage, MockTxArbiter>> {
        const MAX_I_INDEX: u64 = 5;
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
                let _utxos = generate_transaction(tx_id, i);
                i += 1;
                let generated_tx = generate_transaction(tx_id, i);
                debug!("[mock expectations1] generated tx: {:?}", &generated_tx);
                Ok(generated_tx)
            });
            indexer.expect_clone().returning(move || {
                let mut cloned_mocked_indexer = MockTitanIndexer::new();
                let mut i = 0;
                cloned_mocked_indexer.expect_get_transaction().returning(move |tx_id| {
                    let _utxos = generate_transaction(tx_id, i);
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
                |_titan_client: Arc<MockTitanIndexer>, _tx_to_check: &Transaction, tx_info: &TxToUpdateStatus| {
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
                    |_titan_client: Arc<MockTitanIndexer>, _tx_to_check: &Transaction, tx_info: &TxToUpdateStatus| {
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
                        |_titan_client: Arc<MockTitanIndexer>,
                         _tx_to_check: &Transaction,
                         tx_info: &TxToUpdateStatus| {
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
                titan_config: Some(TitanConfig {
                    url: Url::from_str(TITAN_URL)?,
                }),
                maestro_config: None,
                btc_rpc_creds,
                db_pool,
                btc_indexer_params: app_config.btc_indexer_config,
            },
            indexer_client: Arc::new(mocked_indexer),
            tx_validator: Arc::new(mocked_tx_arbiter),
            bitcoin_rpc_client: Arc::new(StubBitcoinRpcClient),
        })?;
        Ok(indexer)
    }
}
