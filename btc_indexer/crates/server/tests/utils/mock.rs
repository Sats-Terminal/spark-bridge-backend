use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc},
};

use crate::utils::init::{DRAFT_TITAN_URL, TEST_LOGGER};
use async_trait::async_trait;
use axum::{Router, routing::post};
use axum_test::TestServer;
use bitcoin::{BlockHash, OutPoint, hashes::Hash};
use bitcoincore_rpc::{bitcoin::Txid};
use btc_indexer_api::api::{BtcIndexerApi, BtcTxReview};
use btc_indexer_internals::indexer::{BtcIndexer, IndexerParams, IndexerParamsWithApi};
use btc_indexer_internals::tx_arbiter::TxArbiterTrait;
use btc_indexer_internals::tx_arbiter::{TxArbiterError, TxArbiterResponse};
use btc_indexer_server::AppState;
use config_parser::config::{BtcRpcCredentials, ServerConfig, TitanConfig};
use global_utils::config_variant::ConfigVariant;
use local_db_store_indexer::schemas::tx_tracking_storage::TxToUpdateStatus;
use local_db_store_indexer::{init::LocalDbStorage};
use mockall::mock;
use persistent_storage::init::{PostgresPool, PostgresRepo};
use reqwest::header::HeaderMap;
use titan_client::{Error, TitanApi};
use titan_types::{
    AddressData, AddressTxOut, Block, BlockTip, InscriptionId, MempoolEntry, Pagination, PaginationResponse,
    RuneResponse, SpentStatus, Status, Subscription, Transaction, TransactionStatus, TxOut, query,
};
use tracing::{debug, info, instrument};
use url::Url;

mock! {
    pub TitanIndexer {}
    impl Clone for TitanIndexer {
        fn clone(&self) -> Self;
    }

    #[async_trait]
    impl TitanApi for TitanIndexer {
        async fn get_status(&self) -> Result<Status, Error>;
        async fn get_tip(&self) -> Result<BlockTip, Error>;
        async fn get_block(&self, query: &query::Block) -> Result<Block, Error>;
        async fn get_block_hash_by_height(&self, height: u64) -> Result<String, Error>;
        async fn get_block_txids(&self, query: &query::Block) -> Result<Vec<String>, Error>;
        async fn get_address(&self, address: &str) -> Result<AddressData, Error>;
        async fn get_transaction(&self, txid: &Txid) -> Result<Transaction, Error>;
        async fn get_transaction_raw(&self, txid: &Txid) -> Result<Vec<u8>, Error>;
        async fn get_transaction_hex(&self, txid: &Txid) -> Result<String, Error>;
        async fn get_transaction_status(&self, txid: &Txid) -> Result<TransactionStatus, Error>;
        async fn send_transaction(&self, tx_hex: String) -> Result<Txid, Error>;
        async fn get_output(&self, outpoint: &OutPoint) -> Result<TxOut, Error>;
        async fn get_inscription(
            &self,
            inscription_id: &InscriptionId,
        ) -> Result<(HeaderMap, Vec<u8>), Error>;
        async fn get_runes(
            &self,
            pagination: Option<Pagination>,
        ) -> Result<PaginationResponse<RuneResponse>, Error>;
        async fn get_rune(&self, rune: &query::Rune) -> Result<RuneResponse, Error>;
        async fn get_rune_transactions(
            &self,
            rune: &query::Rune,
            pagination: Option<Pagination>,
        ) -> Result<PaginationResponse<Txid>, Error>;
        async fn get_mempool_txids(&self) -> Result<Vec<Txid>, Error>;
        async fn get_mempool_entry(&self, txid: &Txid) -> Result<MempoolEntry, Error>;
        async fn get_mempool_entries(
            &self,
            txids: &[Txid],
        ) -> Result<HashMap<Txid, Option<MempoolEntry>>, Error>;
        async fn get_all_mempool_entries(&self) -> Result<HashMap<Txid, MempoolEntry>, Error>;
        async fn get_mempool_entries_with_ancestors(
            &self,
            txids: &[Txid],
        ) -> Result<HashMap<Txid, MempoolEntry>, Error>;
        async fn get_subscription(&self, id: &str) -> Result<Subscription, Error>;
        async fn list_subscriptions(&self) -> Result<Vec<Subscription>, Error>;
        async fn add_subscription(&self, subscription: &Subscription) -> Result<Subscription, Error>;
        async fn delete_subscription(&self, id: &str) -> Result<(), Error>;
    }
}

mock! {
    pub TxArbiter {}
    impl Clone for TxArbiter {
        fn clone(&self) -> Self;
    }

     #[async_trait]
    impl TxArbiterTrait for TxArbiter {
        async fn check_tx<C: TitanApi>(
            &self,
            titan_client: std::sync::Arc<C>,
            tx_to_check: &Transaction,
            tx_info: &TxToUpdateStatus,
        ) -> Result<TxArbiterResponse, TxArbiterError>;
    }
}

const CONFIG_FILEPATH: &str = "../../../infrastructure/configurations/btc_indexer/dev.toml";

#[instrument(
    level = "debug",
    skip(generate_mocked_titan_indexer, generate_mocked_tx_arbiter, pool),
    ret
)]
pub async fn init_mocked_test_server(
    generate_mocked_titan_indexer: impl Fn() -> MockTitanIndexer,
    generate_mocked_tx_arbiter: impl Fn() -> MockTxArbiter,
    pool: PostgresPool,
) -> anyhow::Result<TestServer> {
    let _logger_guard = &*TEST_LOGGER;
    let (btc_creds, config_variant) = (
        BtcRpcCredentials::new()?,
        ConfigVariant::OnlyOneFilepath(CONFIG_FILEPATH.to_string()),
    );
    let app_config = ServerConfig::init_config(config_variant)?;
    let db_pool = LocalDbStorage {
        postgres_repo: PostgresRepo { pool }.into_shared(),
    };
    let mocked_titan_indexer = generate_mocked_titan_indexer();
    let mocked_tx_arbiter = generate_mocked_tx_arbiter();
    let btc_indexer = BtcIndexer::new(IndexerParamsWithApi {
        indexer_params: IndexerParams {
            titan_config: TitanConfig {
                url: Url::from_str(DRAFT_TITAN_URL)?,
            },
            btc_rpc_creds: btc_creds,
            db_pool: db_pool.clone(),
            btc_indexer_params: app_config.btc_indexer_config,
        },
        titan_api_client: Arc::new(mocked_titan_indexer),
        tx_validator: Arc::new(mocked_tx_arbiter),
    })?;

    let app = create_app_mocked(db_pool, btc_indexer).await;
    let test_server = TestServer::builder().http_transport().build(app.into_make_service())?;
    info!("Serving local axum test server on {:?}", test_server.server_address());
    Ok(test_server)
}

#[instrument(level = "trace")]
pub fn generate_mock_titan_indexer_tx_tracking() -> MockTitanIndexer {
    let generate_transaction = |tx_id: &Txid, index: u64| Transaction {
        txid: tx_id.clone(),
        version: 0,
        lock_time: 0,
        input: vec![],
        output: vec![],
        status: TransactionStatus::confirmed(index, BlockHash::all_zeros()),
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
    mocked_indexer
}

#[instrument(level = "trace")]
pub fn generate_mock_tx_arbiter() -> MockTxArbiter {
    let generate_tx_arbiter_mocking_invocations = |tx_arbiter: &mut MockTxArbiter| {
        tx_arbiter.expect_check_tx().returning(
            |titan_client: Arc<MockTitanIndexer>, tx_to_check: &Transaction, tx_info: &TxToUpdateStatus| {
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
                |titan_client: Arc<MockTitanIndexer>, tx_to_check: &Transaction, tx_info: &TxToUpdateStatus| {
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
                    |titan_client: Arc<MockTitanIndexer>, tx_to_check: &Transaction, tx_info: &TxToUpdateStatus| {
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

    let mut arbiter = MockTxArbiter::new();
    generate_tx_arbiter_mocking_invocations(&mut arbiter);
    arbiter
}

#[instrument(skip(db_pool, btc_indexer), level = "trace")]
pub async fn create_app_mocked(
    db_pool: LocalDbStorage,
    btc_indexer: BtcIndexer<MockTitanIndexer, LocalDbStorage, MockTxArbiter>,
) -> Router {
    let (db_pool, btc_indexer) = (Arc::new(db_pool), Arc::new(btc_indexer));
    let state = AppState {
        http_client: reqwest::Client::new(),
        persistent_storage: db_pool,
        btc_indexer,
    };
    let app = Router::new()
        .route(
            BtcIndexerApi::TRACK_TX_ENDPOINT,
            post(btc_indexer_server::routes::track_tx::handler),
        )
        .route(
            BtcIndexerApi::HEALTHCHECK_ENDPOINT,
            post(btc_indexer_server::routes::healthcheck::handler),
        )
        .with_state(state);
    app
}
