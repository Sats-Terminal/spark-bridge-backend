use async_trait::async_trait;
use crate::types::{WatchRunesDepositRequest, WatchRunesDepositResponse, WatchSparkDepositRequest, WatchSparkDepositResponse};
use crate::error::DepositVerificationError;
use std::fmt::Debug;


#[async_trait]
pub trait VerificationClient: Debug + Send + Sync {
    async fn watch_runes_deposit(&self, request: WatchRunesDepositRequest) -> Result<WatchRunesDepositResponse, DepositVerificationError>;
    async fn watch_spark_deposit(&self, request: WatchSparkDepositRequest) -> Result<WatchSparkDepositResponse, DepositVerificationError>;
}
