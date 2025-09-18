use async_trait::async_trait;
use bitcoin::OutPoint;
use bitcoincore_rpc::{RawTx, bitcoin, json};
use btc_indexer_api::api::{Amount, ResponseMeta, VOut};
use serde::{Deserialize, Serialize};
use titan_client::AddressData;
use titan_types::Transaction;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountReplenishmentEvent {
    pub address: String,
    pub account_data: AddressData,
}

pub(crate) struct ChanMsg {
    pub btc_address: String,
    pub out_point: OutPoint,
    pub amount: Amount,
}

#[async_trait]
pub trait BtcIndexerApi: Send + Sync {
    /// Tracks changes of transaction, whether it's confirmed
    async fn check_tx_changes(
        //todo: add Outpoint
        &self,
        out_point: OutPoint,
        amount: Amount,
    ) -> crate::error::Result<Option<ResponseMeta>>;
    fn get_tx_info(&self, tx_id: bitcoin::Txid) -> crate::error::Result<bitcoin::transaction::Transaction>;
    fn get_blockchain_info(&self) -> crate::error::Result<json::GetBlockchainInfoResult>;
    fn broadcast_transaction(&self, tx: impl RawTx) -> crate::error::Result<bitcoin::blockdata::transaction::Txid>;
}
