use crate::error::BtcIndexerClientError;
use bitcoin::OutPoint;
use async_trait::async_trait;
use ordinals::RuneId;
use std::collections::HashMap;
use btc_indexer_config::IndexerClientConfig;

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
pub trait BtcIndexerClientApi {
    fn new(config: IndexerClientConfig) -> Self;
    async fn get_transaction_outpoint(&self, outpoint: OutPoint) -> Result<Option<OutPointData>, BtcIndexerClientError>;
    async fn get_blockchain_info(&self) -> Result<BlockchainInfo, BtcIndexerClientError>;
}
