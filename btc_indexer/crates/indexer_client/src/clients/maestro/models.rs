use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub enum Metaprotocol {
    Inscriptions,
    Runes,
    Brc20,
}

#[derive(Debug, Deserialize)]
pub struct TxInfoResponse {
    pub data: TransactionInfoData,
    pub last_updated: LastUpdatedInfo,
}

#[derive(Debug, Deserialize)]
pub struct LastUpdatedInfo {
    pub block_hash: String,
    pub block_height: u64,
}

#[derive(Debug, Deserialize)]
pub struct TransactionInfoData {
    pub block_hash: String,
    pub confirmations: u64,
    #[serde(with = "serde_str")]
    pub fees: u64,
    pub height: u64,
    pub inputs: Vec<InputVariant>,
    pub metaprotocols: Vec<Metaprotocol>,
    pub outputs: Vec<OutputVariant>,
    pub sats_per_vb: u64,
    pub timestamp: String,
    pub tx_index: u64,
    pub unix_timestamp: u64,
    pub volume: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum InputVariant {
    WithMetaprotocols(InputMetaprotocols),
    Default(Input),
}

#[derive(Debug, Deserialize)]
pub struct InputMetaprotocols {
    #[serde(flatten)]
    pub base: Input,

    pub inscriptions: Vec<Inscription>,
    pub runes: Vec<RuneData>,
}

#[derive(Debug, Deserialize)]
pub struct Input {
    #[serde(with = "serde_str")]
    pub satoshis: u64,
    pub script_pubkey: String,
    pub txid: String,
    pub vout: u64,
    pub address: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OutputVariant {
    WithMetaprotocols(OutputMetaprotocols),
    Default(Output),
}

#[derive(Debug, Deserialize)]
pub struct OutputMetaprotocols {
    #[serde(flatten)]
    pub base: Output,

    pub inscriptions: Vec<Inscription>,
    pub runes: Vec<RuneData>,
}

#[derive(Debug, Deserialize)]
pub struct Output {
    pub address: Option<String>,
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
    pub total_volume: u64,
    pub unix_timestamp: u64,
    pub weight_units: u64,
}

#[derive(Debug, Deserialize)]
pub struct RuneInfoResponse {
    pub data: RuneInfoData,
    pub last_updated: LastUpdatedInfo,
}

#[derive(Debug, Deserialize)]
pub struct RuneInfoData {
    pub divisibility: u64,
    pub etching_cenotaph: bool,
    pub etching_height: u64,
    pub etching_tx: String,
    pub id: String,
    pub max_supply: String,
    pub mints: u64,
    pub name: String,
    pub premine: Option<String>,
    pub spaced_name: String,
    pub symbol: Option<String>,
    pub terms: RuneInfoTerms,
    pub unique_holders: u64,
}

#[derive(Debug, Deserialize)]
pub struct RuneInfoTerms {
    pub amount_per_mint: Option<String>,
    pub end_height: Option<u64>,
    pub end_offset: Option<u64>,
    pub mint_txs_cap: Option<String>,
    pub start_height: Option<u64>,
    pub start_offset: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct RuneUtxoResponse {
    pub data: Vec<RuneUtxoData>,
    pub last_updated: LastUpdatedInfo,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RuneUtxoData {
    pub confirmations: u64,
    pub height: u64,
    pub runes: Vec<RuneData>,
    #[serde(with = "serde_str")]
    pub satoshis: u64,
    pub txid: String,
    pub vout: u32,
}
