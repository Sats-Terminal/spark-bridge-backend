use frost::types::MusigId;
use frost::types::Nonce;
use bitcoin::OutPoint;
use serde::{Deserialize, Serialize};
use gateway_local_db_store::schemas::deposit_address::DepositStatus;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchSparkDepositRequest {
    pub musig_id: MusigId,
    pub nonce: Nonce,
    pub exit_address: String,
    pub amount: u64,
    pub spark_address: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchSparkDepositResponse {
    pub verifier_response: DepositStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositRequest {
    pub musig_id: MusigId,
    pub nonce: Nonce,
    pub amount: u64,
    pub btc_address: String,
    pub bridge_address: String,
    pub out_point: OutPoint,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositResponse {}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NotifyRunesDepositRequest {
    pub verifier_id: u16,
    pub out_point: OutPoint,
    pub sats_fee_amount: u64,
    pub status: DepositStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VerifyRunesDepositRequest {
    pub btc_address: String,
    pub bridge_address: String,
    pub out_point: OutPoint,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VerifySparkDepositRequest {
    pub spark_address: String,
    pub exit_address: String,
}
