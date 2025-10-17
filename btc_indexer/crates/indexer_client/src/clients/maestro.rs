use async_trait::async_trait;
use bitcoin::{OutPoint, Txid};
use btc_indexer_config::MaestroClientConfig;

use crate::{
    client_api::{BlockchainInfo, BtcIndexerClientApi, OutPointData},
    error::BtcIndexerClientError,
};

#[derive(Clone)]
pub struct MaestroClient {}

impl MaestroClient {
    pub fn new(config: &MaestroClientConfig) -> Self {
        Self {}
    }
}

#[async_trait]
impl BtcIndexerClientApi for MaestroClient {
    async fn get_transaction_outpoint(
        &self,
        outpoint: OutPoint,
    ) -> Result<Option<OutPointData>, BtcIndexerClientError> {
        // TODO: implement
        Err(BtcIndexerClientError::InvalidConfigTypeError)
    }

    async fn get_blockchain_info(&self) -> Result<BlockchainInfo, BtcIndexerClientError> {
        // TODO: implement
        Err(BtcIndexerClientError::InvalidConfigTypeError)
    }

    async fn get_block_transactions(&self, block_height: u64) -> Result<Vec<Txid>, BtcIndexerClientError> {
        // TODO: implement
        Err(BtcIndexerClientError::InvalidConfigTypeError)
    }
}
