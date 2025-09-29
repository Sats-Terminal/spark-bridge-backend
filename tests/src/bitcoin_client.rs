use bitcoin::{Address, CompressedPublicKey};
use bitcoin::{Transaction, consensus::Encodable};
use bitcoincore_rpc::bitcoin::Amount as RpcAmount;
use bitcoincore_rpc::{Auth::UserPass, Client, RpcApi};
use thiserror::Error;
use titan_client::{AddressData, TitanApi, TitanClient};
use bitcoin::Network;
use bitcoin::Txid;
use bitcoin::secp256k1::{Secp256k1};
use bitcoin::PrivateKey;
use tracing;
use titan_client::{RuneResponse, query::Block, query::Rune};
use ordinals::RuneId;
use std::str::FromStr;
use std::sync::Arc;
use titan_client::Transaction as TitanTransaction;
use crate::constants::BLOCKS_TO_GENERATE;
use crate::error::BitcoinClientError;

pub struct BitcoinClientConfig {
    pub bitcoin_url: String,
    pub titan_url: String,
    pub bitcoin_username: String,
    pub bitcoin_password: String,
}

#[derive(Clone)]
pub struct BitcoinClient {
    bitcoin_client: Arc<Client>,
    titan_client: Arc<TitanClient>,
}

impl BitcoinClient {
    pub fn new(config: BitcoinClientConfig) -> Result<Self, BitcoinClientError> {
        let bitcoin_client = Client::new(
            config.bitcoin_url.as_str(),
            UserPass(config.bitcoin_username, config.bitcoin_password),
        )?;
        let titan_client = TitanClient::new(config.titan_url.as_str());

        let mut client = Self {
            bitcoin_client: Arc::new(bitcoin_client),
            titan_client: Arc::new(titan_client),
        };
        client.init_bitcoin_faucet_wallet()?;

        Ok(client)
    }

    fn dump_address(&self) -> Result<Address, BitcoinClientError> {
        let private_key = PrivateKey::from_wif("cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN")
            .map_err(|e| BitcoinClientError::DecodeError(e.to_string()))?;
        let compressed_public_key = CompressedPublicKey::from_private_key(&Secp256k1::new(), &private_key)
            .map_err(|e| BitcoinClientError::DecodeError(e.to_string()))?;
        Ok(Address::p2wpkh(&compressed_public_key, Network::Regtest))
    }

    fn init_bitcoin_faucet_wallet(&mut self) -> Result<(), BitcoinClientError> {
        tracing::info!("Initializing bitcoin faucet wallet");
        let wallets = self.bitcoin_client.list_wallets()?;
        if !wallets.contains(&"faucet_wallet".to_string()) {
            tracing::info!("Creating faucet wallet");
            self.bitcoin_client.create_wallet("faucet_wallet", None, None, None, None)?;
            let address = self.get_funding_address()?;
            self.generate_blocks(100, Some(address))?;
            self.generate_blocks(121, None)?;
        }
        Ok(())
    }

    fn get_funding_address(&mut self) -> Result<Address, BitcoinClientError> {
        let address = self.bitcoin_client.get_new_address(None, None)?
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

    pub fn generate_blocks(&mut self, blocks: u64, address: Option<Address>) -> Result<(), BitcoinClientError> {
        tracing::debug!("Generating blocks: {:?}", blocks);
        let address = match address {
            Some(address) => address,
            None => self.dump_address()?,
        };
        self.bitcoin_client
            .generate_to_address(blocks, &address)?;
        Ok(())
    }

    pub fn faucet(&mut self, address: Address, sats_amount: u64) -> Result<(), BitcoinClientError> {
        let amount = RpcAmount::from_sat(sats_amount);
        let txid = self.bitcoin_client
            .send_to_address(&address, amount, None, None, None, None, None, None)?;
        tracing::info!("Fauceted transaction: {:?}", txid);
        self.generate_blocks(BLOCKS_TO_GENERATE, None)?;
        Ok(())
    }

    pub async fn get_address_data(&self, address: Address) -> Result<AddressData, BitcoinClientError> {
        self.titan_client
            .get_address(address.to_string().as_str())
            .await
            .map_err(BitcoinClientError::TitanRpcError)
    }

    pub async fn get_rune_id(&self, txid: &Txid) -> Result<RuneId, BitcoinClientError> {
        let response = self.titan_client.get_transaction(txid).await
            .map_err(BitcoinClientError::TitanRpcError)?;
        let block_height = response.status.block_height
            .ok_or(BitcoinClientError::DecodeError("Block height not found".to_string()))?;
        let block = self.titan_client.get_block(&Block::Height(block_height)).await
            .map_err(BitcoinClientError::TitanRpcError)?;
        let tx_index = block.tx_ids.iter().position(|id| id.to_string() == txid.to_string())
            .ok_or(BitcoinClientError::DecodeError("Transaction not found in block".to_string()))?;
        let rune_id = RuneId::new(block_height, tx_index as u32)
            .ok_or(BitcoinClientError::DecodeError("Rune ID not found".to_string()))?;
        Ok(rune_id)
    }

    pub async fn get_rune(&self, rune_id: String) -> Result<RuneResponse, BitcoinClientError> {
        let rune = Rune::from_str(&rune_id).map_err(|e| BitcoinClientError::DecodeError(e.to_string()))?;
        let response = self.titan_client.get_rune(&rune).await
            .map_err(BitcoinClientError::TitanRpcError)?;
        Ok(response)
    }

    pub async fn get_transaction(&self, txid: &Txid) -> Result<TitanTransaction, BitcoinClientError> {
        let response = self.titan_client.get_transaction(txid).await
            .map_err(BitcoinClientError::TitanRpcError)?;
        Ok(response)
    }
}
