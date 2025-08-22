use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, LazyLock},
};

use async_trait::async_trait;
use axum::{Router, routing::post};
use bitcoin::{BlockHash, OutPoint, hashes::Hash};
use bitcoincore_rpc::{RawTx, bitcoin::Txid};
use btc_indexer_internals::indexer::BtcIndexer;
use btc_indexer_server::AppState;
use config_parser::config::{BtcRpcCredentials, ConfigVariant, PostgresDbCredentials, ServerConfig};
use global_utils::logger::{LoggerGuard, init_logger};
use mockall::mock;
use persistent_storage::init::{PersistentRepoShared, PostgresRepo};
use reqwest::header::HeaderMap;
use titan_client::{Error, TitanApi, TitanClient};
use titan_types::{
    AddressData, AddressTxOut, Block, BlockTip, InscriptionId, MempoolEntry, Pagination, PaginationResponse,
    RuneResponse, SpentStatus, Status, Subscription, Transaction, TransactionStatus, TxOut, query,
};
use tracing::{debug, info, instrument};
use utoipa_swagger_ui::SwaggerUi;

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

#[instrument(skip(db_pool, btc_indexer))]
pub async fn create_app_mocked(db_pool: PersistentRepoShared, btc_indexer: BtcIndexer<MockTitanIndexer>) -> Router {
    let state = AppState {
        http_client: reqwest::Client::new(),
        persistent_storage: db_pool,
        btc_indexer: Arc::new(btc_indexer),
        cached_tasks: Arc::new(Default::default()),
    };
    let app = Router::new()
        .route("/track_tx", post(btc_indexer_server::routes::track_tx::handler))
        .route("/track_wallet", post(btc_indexer_server::routes::track_wallet::handler))
        .with_state(state);
    app
}
