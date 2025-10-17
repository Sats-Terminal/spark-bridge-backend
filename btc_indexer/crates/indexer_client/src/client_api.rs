use async_trait::async_trait;
use bitcoin::{OutPoint, Txid};
use btc_indexer_config::IndexerClientConfig;
use ordinals::RuneId;
use std::collections::HashMap;

use crate::{
    clients::{maestro::MaestroClient, titan::TitanClient},
    error::BtcIndexerClientError,
};

#[derive(Debug, Clone)]
pub struct OutPointData {
    pub outpoint: OutPoint,
    pub block_height: u64,
    pub rune_amounts: HashMap<RuneId, u128>,
    pub sats_amount: u64,
}

#[derive(Debug, Clone)]
pub struct BlockchainInfo {
    pub block_height: u64,
}

#[async_trait]
pub trait BtcIndexerClientApi: Send + Sync {
    async fn get_transaction_outpoint(&self, outpoint: OutPoint)
    -> Result<Option<OutPointData>, BtcIndexerClientError>;
    async fn get_blockchain_info(&self) -> Result<BlockchainInfo, BtcIndexerClientError>;
    async fn get_block_transactions(&self, block_height: u64) -> Result<Vec<Txid>, BtcIndexerClientError>;
}

pub fn new_btc_indexer_client(client_config: IndexerClientConfig) -> Box<dyn BtcIndexerClientApi> {
    match client_config {
        IndexerClientConfig::Titan(cfg) => Box::new(TitanClient::new(&cfg)),
        IndexerClientConfig::Maestro(cfg) => Box::new(MaestroClient::new(&cfg)),
    }
}
