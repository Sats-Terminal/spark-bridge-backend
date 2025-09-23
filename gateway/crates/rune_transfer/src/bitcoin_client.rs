use crate::errors::RuneTransferError;
use bitcoin::Transaction;
use bitcoin::consensus::Encodable;
use bitcoincore_rpc::RpcApi;
use bitcoincore_rpc::{Auth::UserPass, Client};
use gateway_config_parser::config::BitcoinClientConfig;

pub struct BitcoinClient {
    client: Client,
}

impl BitcoinClient {
    pub fn new(config: BitcoinClientConfig) -> Result<Self, RuneTransferError> {
        let client = Client::new(config.url.as_str(), UserPass(config.username, config.password))
            .map_err(|e| RuneTransferError::InvalidData(format!("Failed to create Bitcoin client: {}", e)))?;
        Ok(Self { client })
    }

    pub async fn broadcast_transaction(&self, transaction: Transaction) -> Result<(), RuneTransferError> {
        let mut tx_bytes = Vec::new();
        let _ = transaction
            .consensus_encode(&mut tx_bytes)
            .map_err(|e| RuneTransferError::InvalidData(format!("Failed to encode transaction: {}", e)))?;

        let _ = self
            .client
            .send_raw_transaction(&tx_bytes)
            .map_err(|e| RuneTransferError::InvalidData(format!("Failed to broadcast transaction: {}", e)))?;

        Ok(())
    }
}
