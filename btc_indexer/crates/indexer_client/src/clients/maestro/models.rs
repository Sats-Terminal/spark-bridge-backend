use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub enum Metaprotocol {
    Inscriptions,
    Runes,
    Brc20,
}

#[derive(Debug, Deserialize)]
pub struct TxInfoMetaprotocolsResponse {
    pub data: TransactionData,
    pub last_updated: LastUpdatedInfo,
}

#[derive(Debug, Deserialize)]
pub struct LastUpdatedInfo {
    pub block_hash: String,
    pub block_height: u64,
}

#[derive(Debug, Deserialize)]
pub struct TransactionData {
    pub block_hash: String,
    pub confirmations: u64,
    #[serde(with = "serde_str")]
    pub fees: u64,
    pub height: u64,
    pub inputs: Vec<Input>,
    pub metaprotocols: Vec<Metaprotocol>,
    pub outputs: Vec<Output>,
    pub sats_per_vb: u64,
    pub timestamp: String,
    pub tx_index: u64,
    pub unix_timestamp: u64,
    pub volume: String,
}

#[derive(Debug, Deserialize)]
pub struct Input {
    pub inscriptions: Vec<Inscription>,
    pub runes: Vec<RuneData>,
    #[serde(with = "serde_str")]
    pub satoshis: u64,
    pub script_pubkey: String,
    pub txid: String,
    pub vout: u64,
    pub address: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Output {
    pub address: Option<String>,
    pub inscriptions: Vec<Inscription>,
    pub runes: Vec<RuneData>,
    #[serde(with = "serde_str")]
    pub satoshis: u64,
    pub script_pubkey: String,
    pub spending_tx: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Inscription {
    pub inscription_id: String,
    pub offset: u64,
}

#[derive(Debug, Deserialize)]
pub struct RuneData {
    #[serde(with = "serde_str")]
    pub amount: u64,
    pub rune_id: String,
}

#[derive(Debug, Deserialize)]
pub struct BlockInfoResponse {
    pub data: BlockData,
    pub last_updated: LastUpdatedInfo,
}

#[derive(Debug, Deserialize)]
pub struct BlockData {
    pub coinbase_tag: String,
    pub hash: String,
    pub height: u64,
    pub metaprotocols: Vec<Metaprotocol>,
    pub miner_name: Option<String>,
    pub size: u64,
    pub timestamp: String,
    #[serde(with = "serde_str")]
    pub total_fees: u64,
    pub total_txs: u64,
    #[serde(with = "serde_str")]
    pub total_volume: String,
    pub unix_timestamp: u64,
    pub weight_units: u64,
}
