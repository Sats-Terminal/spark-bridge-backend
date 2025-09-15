use global_utils::common_types::TxIdWrapped;
use serde::{Deserialize, Serialize};
use titan_client::Transaction;
use uuid::Uuid;

/// Callback response from `verifier` to `gateway` that have been checked this tx
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TxCheckCallbackResponse {
    pub identifier: u16,
    pub review_description: Review,
    pub tx: Transaction,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Review {
    Accept,
    Rejected { description: String },
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BridgeRunesToSparkRequest {
    pub uuid: Uuid,
    pub tx: TxIdWrapped,
}

#[derive(Serialize, Debug)]
pub struct BridgeRunesToSparkResponse {
    pub message: String,
}
