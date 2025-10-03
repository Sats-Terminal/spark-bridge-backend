use bitcoin::Address;
use bitcoin::OutPoint;
use frost::types::TweakBytes;
use gateway_local_db_store::schemas::deposit_address::DepositStatus;
use gateway_local_db_store::schemas::user_identifier::UserUniqueId;
use gateway_rune_transfer::transfer::PayingTransferInput;

#[derive(Clone, Debug)]
pub struct WatchSparkDepositRequest {
    pub user_unique_id: UserUniqueId,
    pub nonce: TweakBytes,
    pub exit_address: Address,
    pub amount: u64,
    pub spark_address: String,
}

#[derive(Clone, Debug)]
pub struct WatchSparkDepositResponse {
    pub verifier_response: DepositStatus,
}

#[derive(Clone, Debug)]
pub struct WatchRunesDepositRequest {
    pub user_unique_id: UserUniqueId,
    pub nonce: TweakBytes,
    pub amount: u64,
    pub btc_address: Address,
    pub bridge_address: String,
    pub out_point: OutPoint,
}

#[derive(Clone, Debug)]
pub struct WatchRunesDepositResponse {}

#[derive(Clone, Debug)]
pub struct NotifyRunesDepositRequest {
    pub verifier_id: u16,
    pub out_point: OutPoint,
    pub sats_fee_amount: u64,
    pub status: DepositStatus,
}

#[derive(Clone, Debug)]
pub struct VerifyRunesDepositRequest {
    pub btc_address: Address,
    pub bridge_address: String,
    pub out_point: OutPoint,
}

#[derive(Clone, Debug)]
pub struct VerifySparkDepositRequest {
    pub spark_address: String,
    pub exit_address: Address,
    pub paying_input: PayingTransferInput,
}
