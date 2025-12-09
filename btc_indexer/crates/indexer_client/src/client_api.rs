use std::collections::HashMap;

use async_trait::async_trait;
use bitcoin::{Address, Network, OutPoint, Txid, secp256k1::PublicKey};
use btc_indexer_config::IndexerClientConfig;
use enum_dispatch::enum_dispatch;
use lrc20::token_metadata::TokenMetadata;
use ordinals::RuneId;

use crate::{
    clients::{maestro::MaestroClient, titan::TitanClient},
    error::BtcIndexerClientError,
};

#[derive(Debug, Clone)]
pub struct OutPointData {
    pub outpoint: OutPoint,
    pub block_height: u64,
    pub rune_amounts: HashMap<RuneId, u128>,
    pub sats_amount: u64,
}

#[derive(Debug, Clone)]
pub struct AddrUtxoData {
    pub confirmed: bool,
    pub runes: Vec<RuneData>,
    pub value: u64,
    pub txid: String,
    pub vout: u32,
    // helper field to match both Titan and Maestro
    pub spent: bool,
}

#[derive(Debug, Clone)]
pub struct RuneData {
    pub rune_id: RuneId,
    pub amount: u64,
}

#[derive(Debug, Clone)]
pub struct BlockchainInfo {
    pub block_height: u64,
}

#[async_trait]
#[enum_dispatch]
pub trait BtcIndexer {
    async fn get_transaction_outpoint(&self, outpoint: OutPoint)
    -> Result<Option<OutPointData>, BtcIndexerClientError>;
    async fn get_blockchain_info(&self) -> Result<BlockchainInfo, BtcIndexerClientError>;
    async fn get_block_transactions(&self, block_height: u64) -> Result<Vec<Txid>, BtcIndexerClientError>;
    // Methods mostly used by tests
    async fn get_rune(&self, rune_id: String) -> Result<RuneId, BtcIndexerClientError>;
    async fn get_rune_id(&self, txid: &Txid) -> Result<RuneId, BtcIndexerClientError>;
    async fn get_address_utxos(&self, address: Address) -> Result<Vec<AddrUtxoData>, BtcIndexerClientError>;
}

#[async_trait]
#[enum_dispatch]
pub trait Runer {
    async fn get_rune_metadata(
        &self,
        rune_id: &str,
        issuer_public_key: PublicKey,
        network: Network,
    ) -> Result<TokenMetadata, BtcIndexerClientError>;
}

#[enum_dispatch(BtcIndexer, Runer)]
#[derive(Clone)]
pub enum IndexerClient {
    Titan(TitanClient),
    Maestro(MaestroClient),
}

pub fn new_btc_indexer_client(client_config: IndexerClientConfig) -> IndexerClient {
    match client_config {
        IndexerClientConfig::Titan(cfg) => IndexerClient::Titan(TitanClient::new(&cfg)),
        IndexerClientConfig::Maestro(cfg) => IndexerClient::Maestro(MaestroClient::new(&cfg)),
    }
}
