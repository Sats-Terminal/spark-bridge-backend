use async_trait::async_trait;
use bitcoincore_rpc::{bitcoin, json};

#[async_trait]
pub trait BtcIndexerApi: Send + Sync {
    async fn subscribe(options: Subscription) -> crate::error::Result<SubscriptionEvents>;
    fn get_tx_info(&self, tx_id: bitcoin::Txid) -> crate::error::Result<bitcoin::transaction::Transaction>;
    fn get_blockchain_info(&self) -> crate::error::Result<json::GetBlockchainInfoResult>;
}

#[derive(Debug, Clone, Hash, PartialEq)]
pub enum Subscription {
    SubscribeEvent(),
}

#[derive(Debug, Clone, Hash, PartialEq)]
pub enum SubscriptionEvents {
    SubscribeEventMsg(),
    RuneTransferred {
        amount: u128,
        location: String,
        outpoint: String,
        rune_id: String,
        txid: String,
    },
    TransactionSubmitted {
        txid: String,
        entry: String,
    },
}
