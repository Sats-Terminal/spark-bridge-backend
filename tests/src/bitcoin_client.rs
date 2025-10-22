use bitcoin::{
    Address, CompressedPublicKey, Network, PrivateKey, Transaction, Txid, consensus::Encodable, secp256k1::Secp256k1,
};
use bitcoincore_rpc::{Auth::UserPass, Client, RpcApi, bitcoin::Amount as RpcAmount};
use btc_indexer_client::client_api::{BtcIndexer, IndexerClient, RuneUtxo, new_btc_indexer_client};
use btc_indexer_config::IndexerClientConfig;
use ordinals::RuneId;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing;

use crate::{constants::BLOCKS_TO_GENERATE, error::BitcoinClientError};

pub struct BitcoinClientConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone)]
pub struct BitcoinClient {
    bitcoin_client: Arc<Client>,
    indexer_client: Arc<IndexerClient>,
}

impl BitcoinClient {
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

    pub fn broadcast_transaction(&self, transaction: Transaction) -> Result<(), BitcoinClientError> {
        let mut tx_bytes = Vec::new();
        transaction
            .consensus_encode(&mut tx_bytes)
            .map_err(|e| BitcoinClientError::DecodeError(format!("Failed to encode transaction: {}", e)))?;
        self.bitcoin_client.send_raw_transaction(&tx_bytes)?;
        Ok(())
    }

    pub async fn generate_blocks(&mut self, blocks: u64, address: Option<Address>) -> Result<(), BitcoinClientError> {
        tracing::debug!("Generating blocks: {:?}", blocks);
        let address = match address {
            Some(address) => address,
            None => self.dump_address()?,
        };
        self.bitcoin_client.generate_to_address(blocks, &address)?;
        sleep(Duration::from_secs(1)).await;
        Ok(())
    }

    pub async fn faucet(&mut self, address: Address, sats_amount: u64) -> Result<(), BitcoinClientError> {
        let amount = RpcAmount::from_sat(sats_amount);
        let txid = self
            .bitcoin_client
            .send_to_address(&address, amount, None, None, None, None, None, None)?;
        tracing::info!("Fauceted transaction: {:?}", txid);
        self.generate_blocks(BLOCKS_TO_GENERATE, None).await?;
        Ok(())
    }

    pub async fn get_address_data(&self, address: Address) -> Result<Vec<RuneUtxo>, BitcoinClientError> {
        Ok(self.indexer_client.get_address_rune_utxos(address).await?)
    }

    pub async fn get_rune_id(&self, txid: &Txid) -> Result<RuneId, BitcoinClientError> {
        Ok(self.indexer_client.get_rune_id(txid).await?)
    }

    pub async fn get_rune(&self, rune_id: String) -> Result<RuneId, BitcoinClientError> {
        Ok(self.indexer_client.get_rune(rune_id).await?)
    }

    pub async fn get_transaction(&self, txid: &Txid) -> Result<Transaction, BitcoinClientError> {
        Ok(self
            .bitcoin_client
            .get_transaction(txid, Some(false))
            .map_err(BitcoinClientError::BitcoinRpcError)?
            .transaction()
            .map_err(|e| BitcoinClientError::DecodeError(e.to_string()))?)
    }
}
