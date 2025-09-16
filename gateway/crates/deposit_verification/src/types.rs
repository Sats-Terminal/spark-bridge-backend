use frost::types::MusigId;
use frost::types::Nonce;
use bitcoin::Txid;
use serde::{Deserialize, Serialize};
use gateway_local_db_store::schemas::deposit_address::DepositStatus;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchSparkDepositRequest {
    pub musig_id: MusigId,
    pub nonce: Nonce,
    pub address: String,
    pub amount: u64,
    pub btc_address: String,
    pub txid: Txid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchSparkDepositResponse {
    pub verifier_response: DepositStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositRequest {
    pub musig_id: MusigId,
    pub nonce: Nonce,
    pub address: String,
    pub amount: u64,
    pub btc_address: String,
    pub txid: Txid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositResponse {}
