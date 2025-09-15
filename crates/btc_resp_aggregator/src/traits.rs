use crate::error::BtcAggregatorError;
use async_trait::async_trait;
use bitcoin::Txid;
use global_utils::api_result_request::ApiResponseOwned;
use global_utils::common_types::UrlWrapped;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx;
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CheckTxResponse {
    pub response: ApiResponseOwned<()>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckTxRequest {
    pub tx_id: Txid,
    pub loopback_addr: Url,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxIdStatusValue {
    pub gateway_loopback_addr: UrlWrapped,
    pub status: TxidStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "STATUS_TRANSFERRING")]
pub enum TxidStatus {
    Created,
    Processing,
    Received,
}

#[async_trait]
pub trait TxCheckerClientTrait: Send + Sync {
    /// Response on check request firstly returns Ok(())
    async fn check_tx(&self, req: CheckTxRequest) -> Result<CheckTxResponse, BtcAggregatorError>;
}

#[async_trait]
pub trait BtcTxIdStatusStorage: Send + Sync {
    async fn get_tx_id_value(&self, tx_id: Txid) -> Result<Option<TxIdStatusValue>, DbError>;
    async fn set_tx_id_value(&self, tx_id: Txid, update: &TxIdStatusValue) -> Result<(), DbError>;
    async fn set_tx_id_status(&self, tx_id: Txid, status: &TxidStatus) -> Result<(), DbError>;
}
