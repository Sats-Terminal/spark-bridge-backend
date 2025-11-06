use bitcoin::Address;
use bitcoin::OutPoint;
use frost::types::TweakBytes;
use gateway_local_db_store::schemas::deposit_address::DepositStatus;
use gateway_local_db_store::schemas::user_identifier::UserIds;
use gateway_rune_transfer::transfer::PayingTransferInput;
use serde::Deserialize;
use serde::Serialize;
use token_identifier::TokenIdentifier;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct WatchSparkDepositRequest {
    pub request_id: Uuid,
    pub user_ids: UserIds,
    pub nonce: TweakBytes,
    pub exit_address: Address,
    pub amount: u64,
    pub spark_address: String,
    pub token_identifier: TokenIdentifier,
    pub fee_payment: FeePayment,
}

#[derive(Clone, Debug)]
pub struct WatchSparkDepositResponse {
    pub verifier_response: DepositStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
#[serde(rename_all = "lowercase")]
pub enum FeePayment {
    Btc(OutPoint),
    Spark(String),
}


#[derive(Clone, Debug)]
pub struct WatchRunesDepositRequest {
    pub request_id: Uuid,
    pub user_ids: UserIds,
    pub nonce: TweakBytes,
    pub amount: u64,
    pub btc_address: Address,
    pub bridge_address: String,
    pub outpoint: OutPoint,
    pub fee_payment: FeePayment,
}

#[derive(Clone, Debug)]
pub struct WatchRunesDepositResponse {}

#[derive(Clone, Debug)]
pub struct NotifyRunesDepositRequest {
    pub verifier_id: u16,
    pub request_id: Uuid,
    pub outpoint: OutPoint,
    pub sats_amount: u64,
    pub status: DepositStatus,
    pub error_details: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NotifySparkDepositRequest {
    pub verifier_id: u16,
    pub request_id: Uuid,
    pub spark_address: String,
    pub status: DepositStatus,
    pub error_details: Option<String>,
}

#[derive(Clone, Debug)]
pub struct VerifyRunesDepositRequest {
    pub request_id: Uuid,
    pub btc_address: Address,
    pub bridge_address: String,
    pub outpoint: OutPoint,
    pub fee_payment: FeePayment,
}

#[derive(Clone, Debug)]
pub struct VerifySparkDepositRequest {
    pub request_id: Uuid,
    pub spark_address: String,
    pub paying_input: PayingTransferInput,
    pub fee_payment: FeePayment,
}
