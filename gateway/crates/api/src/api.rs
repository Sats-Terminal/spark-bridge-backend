use serde::{Deserialize, Serialize};
use titan_client::Transaction;

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
