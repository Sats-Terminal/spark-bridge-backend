use bitcoincore_rpc::{Client, Auth::UserPass, RpcApi};
use bitcoincore_rpc::bitcoin::Amount as RpcAmount;
use thiserror::Error;
use bitcoin::{Transaction, consensus::Encodable};
use bitcoin::Address;
use titan_client::{TitanApi, TitanClient, AddressData};

#[derive(Error, Debug)]
pub enum BitcoinClientError {
    #[error("Failed to make bitcoin client call: {0}")]
    BitcoinRpcError(#[from] bitcoincore_rpc::Error),
    #[error("Decode error: {0}")]
    DecodeError(String),
    #[error("Failed to make titan client call: {0}")]
    TitanRpcError(#[from] titan_client::Error),
}

pub struct BitcoinClientConfig {
    pub bitcoin_url: String,
    pub titan_url: String,
    pub bitcoin_username: String,
    pub bitcoin_password: String,
}

pub struct BitcoinClient {
    bitcoin_client: Client,
    titan_client: TitanClient,
    faucet_wallet_address: Option<Address>,
}

impl BitcoinClient {
    pub fn new(config: BitcoinClientConfig) -> Result<Self, BitcoinClientError> {
        let bitcoin_client = Client::new(
            config.bitcoin_url.as_str(),
            UserPass(config.bitcoin_username, config.bitcoin_password)
        )?;
        let titan_client = TitanClient::new(config.titan_url.as_str());

        let mut client = Self { bitcoin_client, titan_client, faucet_wallet_address: None };
        client.init_bitcoin_faucet_wallet()?;

        Ok(client)
    }

    fn get_faucet_wallet_address(&mut self) -> Result<Address, BitcoinClientError> {
        match self.faucet_wallet_address.clone() {
            Some(address) => Ok(address),
            None => {
                let address = self.bitcoin_client.get_new_address(None, None)?.assume_checked();
                self.faucet_wallet_address = Some(address.clone());
                Ok(address)
            }
        }
    }

    fn init_bitcoin_faucet_wallet(&mut self) -> Result<(), BitcoinClientError> {
        let wallets = self.bitcoin_client.list_wallets()?;
        if !wallets.contains(&"faucet_wallet".to_string()) {
            self.generate_blocks(121)?;
        }
        Ok(())
    }

    pub fn broadcast_transaction(&self, transaction: Transaction) -> Result<(), BitcoinClientError> {
        let mut tx_bytes = Vec::new();
        transaction.consensus_encode(&mut tx_bytes)
            .map_err(|e| BitcoinClientError::DecodeError(e.to_string()))?;
        self.bitcoin_client.send_raw_transaction(&tx_bytes)?;
        Ok(())
    }

    pub fn generate_blocks(&mut self, blocks: u64) -> Result<(), BitcoinClientError> {
        let faucet_wallet_address = self.get_faucet_wallet_address()?;
        self.bitcoin_client.generate_to_address(blocks, &faucet_wallet_address)?;
        Ok(())
    }

    pub fn faucet(&mut self, address: Address, sats_amount: u64) -> Result<(), BitcoinClientError> {
        let amount = RpcAmount::from_sat(sats_amount);
        self.bitcoin_client.send_to_address(&address, amount, None, None, None, None, None, None)?;
        self.generate_blocks(6)?;
        Ok(())
    }

    pub async fn get_address_data(&self, address: Address) -> Result<AddressData, BitcoinClientError> {
        self.titan_client.get_address(address.to_string().as_str()).await
            .map_err(BitcoinClientError::TitanRpcError)
    }
}
