use std::{collections::HashMap, str::FromStr};

use async_trait::async_trait;
use bitcoin::{OutPoint, hashes::Hash};
use bitcoincore_rpc::{RawTx, bitcoin::Txid};
use btc_indexer_internals::tx_arbiter::TxArbiterTrait;
use btc_indexer_internals::tx_arbiter::{TxArbiterError, TxArbiterResponse};
use local_db_store_indexer::schemas::tx_tracking_storage::TxToUpdateStatus;
use mockall::mock;
use reqwest::header::HeaderMap;
use titan_client::{Error, TitanApi};
use titan_types::{
    AddressData, Block, BlockTip, InscriptionId, MempoolEntry, Pagination, PaginationResponse, RuneResponse, Status,
    Subscription, Transaction, TransactionStatus, TxOut, query,
};

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
