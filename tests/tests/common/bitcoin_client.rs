use async_trait::async_trait;
use bitcoin::{
    Address, CompressedPublicKey, Network, PrivateKey, Transaction, Txid, consensus::Encodable, secp256k1::Secp256k1,
};
use bitcoincore_rpc::{Auth::UserPass, Client, RawTx, RpcApi, bitcoin::Amount as RpcAmount};
use btc_indexer_client::client_api::{AddrUtxoData, BtcIndexer, IndexerClient, new_btc_indexer_client};
use btc_indexer_config::IndexerClientConfig;
use ordinals::RuneId;
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing;
use url::Url;

use crate::common::{constants::BLOCKS_TO_GENERATE, error::BitcoinClientError};

pub struct BitcoinClientConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}

#[async_trait]
pub trait BitcoinClient {
    async fn broadcast_transaction(&self, transaction: Transaction) -> Result<(), BitcoinClientError>;
    async fn generate_blocks(&mut self, _: u64, _: Option<Address>) -> Result<(), BitcoinClientError>;
    async fn faucet(&mut self, _: Address, _: u64) -> Result<(), BitcoinClientError>;
    async fn get_address_data(&self, address: Address) -> Result<Vec<AddrUtxoData>, BitcoinClientError>;
    async fn get_rune_id(&self, txid: &Txid) -> Result<RuneId, BitcoinClientError>;
    async fn get_rune(&self, rune_id: String) -> Result<RuneId, BitcoinClientError>;
    async fn wait_mined(&self, txid: &Txid) -> Result<(), BitcoinClientError>;
}

#[derive(Clone)]
pub struct BitcoinRegtestClient {
    bitcoin_client: Arc<Client>,
    indexer_client: Arc<IndexerClient>,
}

impl BitcoinRegtestClient {
    pub async fn new(
        btc_cfg: BitcoinClientConfig,
        indexer_cfg: IndexerClientConfig,
    ) -> Result<Self, BitcoinClientError> {
        let bitcoin_client = Client::new(&btc_cfg.url, UserPass(btc_cfg.username, btc_cfg.password))?;
        let indexer_client = new_btc_indexer_client(indexer_cfg);

        let mut client = Self {
            bitcoin_client: Arc::new(bitcoin_client),
            indexer_client: Arc::new(indexer_client),
        };
        client.init_bitcoin_faucet_wallet().await?;

        Ok(client)
    }

    fn dump_address(&self) -> Result<Address, BitcoinClientError> {
        let private_key = PrivateKey::from_wif("cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN")
            .map_err(|e| BitcoinClientError::DecodeError(e.to_string()))?;
        let compressed_public_key = CompressedPublicKey::from_private_key(&Secp256k1::new(), &private_key)
            .map_err(|e| BitcoinClientError::DecodeError(e.to_string()))?;
        Ok(Address::p2wpkh(&compressed_public_key, Network::Regtest))
    }

    async fn init_bitcoin_faucet_wallet(&mut self) -> Result<(), BitcoinClientError> {
        tracing::info!("Initializing bitcoin faucet wallet");
        let wallets = self.bitcoin_client.list_wallets()?;
        if !wallets.contains(&"faucet_wallet".to_string()) {
            tracing::info!("Creating faucet wallet");
            self.bitcoin_client
                .create_wallet("faucet_wallet", None, None, None, None)?;
            let address = self.get_funding_address()?;
            self.generate_blocks(100, Some(address)).await?;
            self.generate_blocks(121, None).await?;
        }
        Ok(())
    }

    fn get_funding_address(&mut self) -> Result<Address, BitcoinClientError> {
        let address = self
            .bitcoin_client
            .get_new_address(None, None)?
            .require_network(Network::Regtest)
            .map_err(|e| BitcoinClientError::DecodeError(e.to_string()))?;
        Ok(address)
    }
}

#[async_trait]
impl BitcoinClient for BitcoinRegtestClient {
    async fn broadcast_transaction(&self, transaction: Transaction) -> Result<(), BitcoinClientError> {
        let mut tx_bytes = Vec::new();
        transaction
            .consensus_encode(&mut tx_bytes)
            .map_err(|e| BitcoinClientError::DecodeError(format!("Failed to encode transaction: {}", e)))?;
        self.bitcoin_client.send_raw_transaction(&tx_bytes)?;
        Ok(())
    }

    async fn generate_blocks(&mut self, blocks: u64, address: Option<Address>) -> Result<(), BitcoinClientError> {
        tracing::debug!("Generating blocks: {:?}", blocks);
        let address = match address {
            Some(address) => address,
            None => self.dump_address()?,
        };
        self.bitcoin_client.generate_to_address(blocks, &address)?;
        sleep(Duration::from_secs(1)).await;
        Ok(())
    }

    async fn faucet(&mut self, address: Address, sats_amount: u64) -> Result<(), BitcoinClientError> {
        let amount = RpcAmount::from_sat(sats_amount);
        let txid = self
            .bitcoin_client
            .send_to_address(&address, amount, None, None, None, None, None, None)?;
        tracing::info!("Fauceted transaction: {:?}", txid);
        self.generate_blocks(BLOCKS_TO_GENERATE, None).await?;
        Ok(())
    }

    async fn get_address_data(&self, address: Address) -> Result<Vec<AddrUtxoData>, BitcoinClientError> {
        Ok(self.indexer_client.get_address_utxos(address).await?)
    }

    async fn get_rune_id(&self, txid: &Txid) -> Result<RuneId, BitcoinClientError> {
        Ok(self.indexer_client.get_rune_id(txid).await?)
    }

    async fn get_rune(&self, rune_id: String) -> Result<RuneId, BitcoinClientError> {
        Ok(self.indexer_client.get_rune(rune_id).await?)
    }

    async fn wait_mined(&self, txid: &Txid) -> Result<(), BitcoinClientError> {
        let poll_interval = Duration::from_secs(10);

        tracing::info!(?txid, "Waiting for transaction to be mined");

        loop {
            match self.bitcoin_client.get_raw_transaction_info(&txid, None) {
                Ok(tx_info) => {
                    if let Some(confirmations) = tx_info.confirmations {
                        if confirmations > 0 {
                            tracing::info!(?txid, ?confirmations, "Transaction confirmed");
                            return Ok(());
                        }
                    } else {
                        tracing::debug!(?txid, "Transaction hasn't been confirmed, waiting ...");
                    }
                }
                Err(err) => {
                    tracing::error!(?txid, ?err, "Failed to get transaction");
                    return Err(BitcoinClientError::DecodeError(err.to_string()));
                }
            }
            sleep(poll_interval).await;
        }
    }
}

#[derive(Clone)]
pub struct BitcoinTestnetClient {
    base_url: Url,
    api_client: reqwest::Client,
    indexer_client: Arc<IndexerClient>,
}

impl BitcoinTestnetClient {
    pub fn new(base_url: Url, indexer_cfg: IndexerClientConfig) -> Self {
        Self {
            base_url,
            api_client: reqwest::Client::new(),
            indexer_client: Arc::new(new_btc_indexer_client(indexer_cfg)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TxStatus {
    confirmed: bool,
}

#[derive(Debug, Deserialize)]
struct TxInfo {
    txid: String,
    status: TxStatus,
}

#[async_trait]
impl BitcoinClient for BitcoinTestnetClient {
    async fn broadcast_transaction(&self, transaction: Transaction) -> Result<(), BitcoinClientError> {
        tracing::warn!(tx = transaction.raw_hex(), "Transaction hex");
        let url = self
            .base_url
            .join("tx")
            .map_err(|e| BitcoinClientError::DecodeError(e.to_string()))?
            .to_string();
        let response = self
            .api_client
            .post(url.clone())
            .body(transaction.raw_hex())
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "N/A".to_string());

            tracing::error!(url, status = status.as_str(), body, "Failed to do request");
            return Err(BitcoinClientError::DecodeError(
                "Failed to broadcast raw transaction".to_string(),
            ));
        }
        // Give some time to broadcast tx
        tracing::debug!("Sleep for 30s to give some time for transaction to broadcast");
        sleep(Duration::from_secs(30)).await;
        Ok(())
    }

    async fn generate_blocks(&mut self, _: u64, _: Option<Address>) -> Result<(), BitcoinClientError> {
        // TODO: sleep?
        Ok(())
    }

    async fn faucet(&mut self, _: Address, _: u64) -> Result<(), BitcoinClientError> {
        // to keep consistency between methods
        Ok(())
    }

    async fn get_address_data(&self, address: Address) -> Result<Vec<AddrUtxoData>, BitcoinClientError> {
        Ok(self.indexer_client.get_address_utxos(address).await?)
    }

    async fn get_rune_id(&self, txid: &Txid) -> Result<RuneId, BitcoinClientError> {
        Ok(self.indexer_client.get_rune_id(txid).await?)
    }

    async fn get_rune(&self, rune_id: String) -> Result<RuneId, BitcoinClientError> {
        Ok(self.indexer_client.get_rune(rune_id).await?)
    }

    async fn wait_mined(&self, txid: &Txid) -> Result<(), BitcoinClientError> {
        let poll_interval = Duration::from_secs(60);
        let txid_url = format!("tx/{}", txid.to_string());
        let url = self
            .base_url
            .join(&txid_url)
            .map_err(|e| BitcoinClientError::DecodeError(e.to_string()))?
            .to_string();
        tracing::info!(?txid, "Waiting for transaction to be mined");

        loop {
            let response = self.api_client.get(&url).send().await?;
            if response.status().is_success() {
                let body_str = response.text().await?;
                tracing::debug!(bodsy=?body_str, "Resp body");
                match serde_json::from_str::<TxInfo>(&body_str) {
                    Ok(tx_info) => {
                        if tx_info.status.confirmed {
                            tracing::info!(?txid, "Transaction confirmed");
                            return Ok(());
                        }
                        tracing::debug!(?txid, "Transaction hasn't been confirmed, waiting ...");
                        sleep(poll_interval).await;
                        continue;
                    }
                    Err(err) => {
                        tracing::error!(?txid, ?err, "Failed to parse response");
                        return Err(BitcoinClientError::DecodeError(err.to_string()));
                    }
                }
            }

            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "N/A".to_string());
            tracing::warn!(url, status = status.as_str(), body, "Failed to do request");
            return Err(BitcoinClientError::DecodeError("Failed to do request".to_string()));
        }
    }
}
