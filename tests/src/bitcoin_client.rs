use bitcoin::Address;
use bitcoin::{Transaction, consensus::Encodable};
use bitcoincore_rpc::bitcoin::Amount as RpcAmount;
use bitcoincore_rpc::{Auth::UserPass, Client, RpcApi};
use thiserror::Error;
use titan_client::{AddressData, TitanApi, TitanClient};

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
}

impl BitcoinClient {
    pub fn new(config: BitcoinClientConfig) -> Result<Self, BitcoinClientError> {
        let bitcoin_client = Client::new(
            config.bitcoin_url.as_str(),
            UserPass(config.bitcoin_username, config.bitcoin_password),
        )?;

        let titan_client = TitanClient::new(config.titan_url.as_str());

        Ok(Self {
            bitcoin_client,
            titan_client,
        })
    }

    pub fn init_bitcoin_faucet_wallet(&self) -> Result<(), BitcoinClientError> {
        // self.bitcoin_client.create_wallet("faucet_wallet", None, None, None, None)?;
        self.bitcoin_client.generate(151, None)?;
        Ok(())
    }

    pub fn broadcast_transaction(&self, transaction: Transaction) -> Result<(), BitcoinClientError> {
        let mut tx_bytes = Vec::new();
        transaction
            .consensus_encode(&mut tx_bytes)
            .map_err(|e| BitcoinClientError::DecodeError(e.to_string()))?;
        self.bitcoin_client.send_raw_transaction(&tx_bytes)?;
        Ok(())
    }

    pub fn generate_blocks(&self, blocks: u64) -> Result<(), BitcoinClientError> {
        self.bitcoin_client.generate(blocks, None)?;
        Ok(())
    }

    pub fn faucet(&self, address: Address, sats_amount: u64) -> Result<(), BitcoinClientError> {
        let amount = RpcAmount::from_sat(sats_amount);
        self.bitcoin_client
            .send_to_address(&address, amount, None, None, None, None, None, None)?;
        self.bitcoin_client.generate(6, None)?;
        Ok(())
    }

    pub async fn get_address_data(&self, address: Address) -> Result<AddressData, BitcoinClientError> {
        self.titan_client
            .get_address(address.to_string().as_str())
            .await
            .map_err(BitcoinClientError::TitanRpcError)
    }
}
