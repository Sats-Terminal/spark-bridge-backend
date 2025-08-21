use async_trait::async_trait;
use bitcoincore_rpc::{RawTx, bitcoin, json};
use titan_client::AddressData;
use titan_types::Transaction;

#[derive(Debug)]
pub struct AccountReplenishmentEvent {
    pub address: String,
    pub account_data: AddressData,
}

#[async_trait]
pub trait BtcIndexerApi: Send + Sync {
    /// Tracks changes of transaction, whether it's confirmed
    fn track_tx_changes(
        &self,
        tx_id: bitcoin::Txid,
    ) -> crate::error::Result<tokio::sync::oneshot::Receiver<Transaction>>;
    /// Tracks changes of runes balance on account (from empty -> to filled with checked utxos )
    fn track_account_changes(
        &self,
        tx_id: impl AsRef<str>,
    ) -> crate::error::Result<tokio::sync::oneshot::Receiver<AccountReplenishmentEvent>>;
    fn get_tx_info(&self, tx_id: bitcoin::Txid) -> crate::error::Result<bitcoin::transaction::Transaction>;
    fn get_blockchain_info(&self) -> crate::error::Result<json::GetBlockchainInfoResult>;
    fn broadcast_transaction(&self, tx: impl RawTx) -> crate::error::Result<bitcoin::blockdata::transaction::Txid>;
}
