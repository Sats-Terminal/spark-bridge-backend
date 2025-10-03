use async_trait::async_trait;
use bitcoincore_rpc::{RawTx, bitcoin, json};
use btc_indexer_api::api::TrackTxRequest;
use serde::{Deserialize, Serialize};
use titan_client::AddressData;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountReplenishmentEvent {
    pub address: String,
    pub account_data: AddressData,
}

#[async_trait]
pub trait BtcIndexerApi: Send + Sync {
    /// Tracks changes of transaction, whether it's confirmed
    async fn check_tx_changes(&self, uuid: Uuid, payload: &TrackTxRequest) -> crate::error::Result<()>;
    async fn healthcheck(&self) -> crate::error::Result<()>;
    fn get_tx_info(&self, tx_id: bitcoin::Txid) -> crate::error::Result<bitcoin::transaction::Transaction>;
    fn get_blockchain_info(&self) -> crate::error::Result<json::GetBlockchainInfoResult>;
    fn broadcast_transaction(&self, tx: impl RawTx) -> crate::error::Result<bitcoin::blockdata::transaction::Txid>;
}
