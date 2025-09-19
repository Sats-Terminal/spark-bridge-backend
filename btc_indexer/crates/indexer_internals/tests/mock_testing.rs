mod utils;

static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());

use std::{str::FromStr, sync::LazyLock};

use bitcoin::{BlockHash, hashes::Hash};
use bitcoincore_rpc::{RawTx, bitcoin::Txid};
use btc_indexer_internals::{
    api::BtcIndexerApi,
    indexer::{BtcIndexer, IndexerParams, IndexerParamsWithApi},
};
use config_parser::config::{BtcRpcCredentials, ServerConfig};
use global_utils::{
    common_types::get_uuid,
    logger::{LoggerGuard, init_logger},
};
use local_db_store_indexer::PostgresDbCredentials;
use titan_client::TitanApi;
use titan_types::{AddressData, AddressTxOut, SpentStatus, Transaction, TransactionStatus};
use tracing::debug;

use crate::utils::{
    common::{compare_address_tx, compare_address_tx_outs},
    mock::MockTitanIndexer,
};

mod mock_testing {
    use btc_indexer_api::api::TrackTxRequest;
    use global_utils::config_variant::ConfigVariant;
    use local_db_store_indexer::init::LocalDbStorage;

    use super::*;

    // Test requires to run Postgres & Docker files (bitcoind + titan)
    #[ignore]
    #[tokio::test]
    async fn init_btc_indexer() -> anyhow::Result<()> {
        dotenv::dotenv()?;
        let _logger_guard = &*TEST_LOGGER;
        let btc_rpc_creds = BtcRpcCredentials::new()?;
        let db_pool = LocalDbStorage::from_config(PostgresDbCredentials::from_envs()?).await?;
        let app_config = ServerConfig::init_config(ConfigVariant::Local)?;
        let indexer = BtcIndexer::with_api(IndexerParams {
            btc_rpc_creds,
            db_pool,
            btc_indexer_params: app_config.btc_indexer_config,
        })?;
        println!("Blockchain info: {:?}", indexer.get_blockchain_info()?);
        Ok(())
    }

    #[tokio::test]
    async fn test_retrieving_of_finalized_tx() -> anyhow::Result<()> {
        const MAX_I_INDEX: u64 = 5;

        let tx_id = Txid::from_str("f74516e3b24af90fc2da8251d2c1e3763252b15c7aec3c1a42dde7116138caee")?;
        let uuid = get_uuid();

        dotenv::dotenv()?;
        let _logger_guard = &*TEST_LOGGER;
        let btc_rpc_creds = BtcRpcCredentials::new()?;
        let db_pool = LocalDbStorage::from_config(PostgresDbCredentials::from_envs()?).await?;
        let app_config = ServerConfig::init_config(ConfigVariant::Local)?;

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
        let generate_mocking_invocations = |indexer: &mut MockTitanIndexer| {
            let mut i = 0;
            indexer.expect_get_transaction().returning(move |tx_id| {
                let utxos = generate_transaction(tx_id, i);
                i += 1;
                Ok(generate_transaction(tx_id, i))
            });
            indexer.expect_clone().returning(move || {
                let mut cloned_mocked_indexer = MockTitanIndexer::new();
                let mut i = 0;
                cloned_mocked_indexer.expect_get_transaction().returning(move |tx_id| {
                    let utxos = generate_transaction(tx_id, i);
                    i += 1;
                    Ok(generate_transaction(tx_id, i))
                });
                cloned_mocked_indexer
                    .expect_clone()
                    .returning(|| MockTitanIndexer::new());
                cloned_mocked_indexer
            });
        };

        debug!("Initializing mocked indexer");
        let mut mocked_indexer = MockTitanIndexer::new();
        generate_mocking_invocations(&mut mocked_indexer);
        debug!("Building BtcIndexer...");
        let indexer = BtcIndexer::new(IndexerParamsWithApi {
            indexer_params: IndexerParams {
                btc_rpc_creds,
                db_pool,
                btc_indexer_params: app_config.btc_indexer_config,
            },
            titan_api_client: mocked_indexer,
        })?;
        debug!("Tracking tx changes..");
        let oneshot = indexer
            .check_tx_changes(
                uuid,
                TrackTxRequest {
                    callback_url: UrlWrapped(),
                    btc_address: "".to_string(),
                    out_point: Default::default(),
                    amount: 0,
                },
            )
            .await?;
        debug!("Receiving oneshot event..");
        let result = oneshot.await?;
        debug!("Event: {result:?}");
        assert!(compare_address_tx(&result, &generate_transaction(&tx_id, MAX_I_INDEX)));
        Ok(())
    }
}
