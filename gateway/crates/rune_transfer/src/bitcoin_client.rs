use async_trait::async_trait;
use bitcoin::{Transaction, consensus::Encodable};
use bitcoincore_rpc::{Auth::UserPass, Client, RawTx, RpcApi};
use enum_dispatch::enum_dispatch;
use gateway_config_parser::config::{BitcoinClientConfig, BitcoinNodeAuth};
use tracing::error;
use url::Url;

use crate::errors::RuneTransferError;

#[async_trait]
#[enum_dispatch]
pub trait Broadcaster {
    async fn broadcast_transaction(&self, transaction: &Transaction) -> Result<(), RuneTransferError>;
}

#[enum_dispatch(Broadcaster)]
pub enum BitcoinClient {
    Node(BitcoinNodeClient),
    Api(BitcoinApiClient),
}

pub fn new_bitcoin_client(cfg: BitcoinClientConfig) -> Result<BitcoinClient, RuneTransferError> {
    Ok(match cfg.auth {
        Some(auth) => BitcoinClient::Node(BitcoinNodeClient::new(&cfg.url, auth)?),
        None => BitcoinClient::Api(BitcoinApiClient::new(&cfg.url)?),
    })
}

pub struct BitcoinNodeClient {
    client: bitcoincore_rpc::Client,
}

impl BitcoinNodeClient {
    pub fn new(url: &str, auth: BitcoinNodeAuth) -> Result<Self, RuneTransferError> {
        let client = Client::new(url, UserPass(auth.username, auth.password))
            .map_err(|e| RuneTransferError::InvalidData(format!("Failed to create Bitcoin client: {}", e)))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl Broadcaster for BitcoinNodeClient {
    async fn broadcast_transaction(&self, transaction: &Transaction) -> Result<(), RuneTransferError> {
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

pub struct BitcoinApiClient {
    base_url: Url,
    client: reqwest::Client,
}

impl BitcoinApiClient {
    pub fn new(url: &str) -> Result<Self, RuneTransferError> {
        Ok(Self {
            base_url: Url::parse(url)?,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl Broadcaster for BitcoinApiClient {
    async fn broadcast_transaction(&self, transaction: &Transaction) -> Result<(), RuneTransferError> {
        let url = self
            .base_url
            .join("/tx")
            .map_err(|err| RuneTransferError::InvalidData(err.to_string()))?
            .to_string();

        let response = self.client.post(url.clone()).body(transaction.raw_hex()).send().await?;
        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| "N/A".to_string());

        error!(url, status = status.as_str(), body, "Failed to do request");
        Err(RuneTransferError::InvalidData(
            "Broadcast tx request failed".to_string(),
        ))
    }
}
