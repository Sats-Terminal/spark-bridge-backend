use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Network {
    Regtest,
    Mainnet,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParticipantType {
    Spark,
    Lightning,
    Bitcoin,
    Issuer,
    BurnAddress,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TxStatus {
    Confirmed,
    Pending,
    Sent,
    Failed,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxV1Response {
    pub id: String,
    #[serde(rename = "type")]
    pub tx_type: String,
    pub status: TxStatus,
    pub amount_sats: u64,
    pub value_usd: f64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub from: Participant,
    pub to: Participant,
    pub time_taken_seconds: Option<f64>,
    pub txid: Option<String>,
    pub bitcoin_tx_data: Option<BitcoinTxData>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    #[serde(rename = "type")]
    pub p_type: ParticipantType,
    pub identifier: String,
    pub pubkey: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitcoinTxData {
    pub txid: String,
    pub vin: Vec<Input>,
    pub vout: Vec<Output>,
    pub status: Option<BitcoinTxDataStatus>,
    pub fee: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub value: i64,
    pub prevout: Option<String>,
    pub witness: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub value: i64,
    pub scriptpubkey_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitcoinTxDataStatus {
    pub confirmed: bool,
    pub block_height: Option<i64>,
    pub block_time: Option<i64>,
}
