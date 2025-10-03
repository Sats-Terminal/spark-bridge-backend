use crate::error::DepositVerificationError;
use crate::types::{
    WatchRunesDepositRequest, WatchRunesDepositResponse, WatchSparkDepositRequest, WatchSparkDepositResponse,
};
use async_trait::async_trait;
use std::fmt::Debug;

pub trait DepositVerificationClientTrait: VerificationClient + VerificationClientHealthCheck {}

#[async_trait]
pub trait VerificationClient: Debug + Send + Sync {
    async fn watch_runes_deposit(
        &self,
        request: WatchRunesDepositRequest,
    ) -> Result<WatchRunesDepositResponse, DepositVerificationError>;
    async fn watch_spark_deposit(
        &self,
        request: WatchSparkDepositRequest,
    ) -> Result<WatchSparkDepositResponse, DepositVerificationError>;
}

#[async_trait]
pub trait VerificationClientHealthCheck {
    async fn healthcheck(&self) -> Result<(), DepositVerificationError>;
}
