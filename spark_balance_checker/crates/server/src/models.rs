use serde::{Deserialize, Serialize};
use token_identifier::TokenIdentifier;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyBalanceRequest {
    pub request_id: Uuid,
    pub spark_address: String,
    pub token_identifier: TokenIdentifier,
    pub amount: u128,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DepositStatus {
    Confirmed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VerificationErrorDetails {
    InvalidTransfer(String),
    TokenIdentifierNotFound(String),
    BalanceMismatch(String),
}

impl ToString for VerificationErrorDetails {
    fn to_string(&self) -> String {
        match self {
            VerificationErrorDetails::InvalidTransfer(msg) => msg.clone(),
            VerificationErrorDetails::TokenIdentifierNotFound(msg) => msg.clone(),
            VerificationErrorDetails::BalanceMismatch(msg) => msg.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyBalanceResponse {
    pub deposit_status: DepositStatus,
    pub error_details: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyTransferRequest {
    pub request_id: Uuid,
    pub receiver_address: String,
    pub txid: String,
    pub amount: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyTransactionResponse {
    pub deposit_status: DepositStatus,
    pub error_details: Option<String>,
}
