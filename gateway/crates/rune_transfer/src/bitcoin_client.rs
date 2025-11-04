use crate::errors::RuneTransferError;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::{Network, Transaction};
use bitcoin_rpc_client::{BitcoinRpcClient, CoreBitcoinRpcClient, MaestroBitcoinRpcClient};
use bitcoincore_rpc::Auth;
use gateway_config_parser::config::BitcoinClientConfig;
use std::env;
use std::sync::Arc;
use url::Url;

pub struct BitcoinClient {
    client: Arc<dyn BitcoinRpcClient>,
}

impl BitcoinClient {
    pub fn new(config: BitcoinClientConfig, network: Network) -> Result<Self, RuneTransferError> {
        let client: Arc<dyn BitcoinRpcClient> = if matches!(network, Network::Regtest) {
            let auth = Auth::UserPass(config.username.clone(), config.password.clone());
            let rpc = CoreBitcoinRpcClient::new(config.url.as_str(), auth)
                .map_err(|e| RuneTransferError::InvalidData(format!("Failed to create Bitcoin client: {e}")))?;
            Arc::new(rpc)
        } else {
            let _ = (&config.url, &config.username, &config.password);
            let base_url = env::var("MAESTRO_API_URL").map_err(|_| {
                RuneTransferError::InvalidData("MAESTRO_API_URL environment variable is not set".to_string())
            })?;
            let api_key = env::var("MAESTRO_API_KEY").map_err(|_| {
                RuneTransferError::InvalidData("MAESTRO_API_KEY environment variable is not set".to_string())
            })?;

            let rpc = MaestroBitcoinRpcClient::new(
                Url::parse(&base_url)
                    .map_err(|e| RuneTransferError::InvalidData(format!("Invalid MAESTRO_API_URL: {e}")))?,
                api_key,
            )
            .map_err(|e| RuneTransferError::InvalidData(format!("Failed to create Maestro client: {e}")))?;
            Arc::new(rpc)
        };

        Ok(Self { client })
    }

    pub async fn broadcast_transaction(&self, transaction: Transaction) -> Result<(), RuneTransferError> {
        let tx_hex = serialize_hex(&transaction);
        self.client
            .send_raw_transaction(&tx_hex)
            .map_err(|e| RuneTransferError::InvalidData(format!("Failed to broadcast transaction: {e}")))?;
        Ok(())
    }
}
