use crate::errors::AggregatorError;
use crate::types::*;
use async_trait::async_trait;
use persistent_storage::error::DbError;
use persistent_storage::init::StorageHealthcheck;
use uuid::Uuid;

#[async_trait]
pub trait SignerClient: Send + Sync {
    async fn dkg_round_1(&self, request: DkgRound1Request) -> Result<DkgRound1Response, AggregatorError>;
    async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response, AggregatorError>;
    async fn dkg_finalize(&self, request: DkgFinalizeRequest) -> Result<DkgFinalizeResponse, AggregatorError>;
    async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response, AggregatorError>;
    async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response, AggregatorError>;
}

#[async_trait]
pub trait AggregatorDkgShareStorage: Send + Sync + StorageHealthcheck {
    async fn get_dkg_share_agg_data(
        &self,
        dkg_share_id: &Uuid,
    ) -> Result<Option<AggregatorDkgShareData>, DbError>;
    async fn set_dkg_share_agg_data(
        &self,
        dkg_share_id: &Uuid,
        dkg_share_data: AggregatorDkgShareData,
    ) -> Result<(), DbError>;
}

#[async_trait]
pub trait AggregatorSignSessionStorage: Send + Sync + StorageHealthcheck {
    async fn get_sign_data(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
    ) -> Result<Option<AggregatorSignData>, DbError>;
    async fn set_sign_data(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
        sign_session_data: AggregatorSignData,
    ) -> Result<(), DbError>;
}

#[async_trait]
pub trait SignerDkgShareStorage: Send + Sync + StorageHealthcheck {
    async fn get_dkg_share_signer_data(
        &self,
        dkg_share_id: &Uuid,
    ) -> Result<Option<SignerDkgShareIdData>, DbError>;
    async fn set_dkg_share_signer_data(
        &self,
        dkg_share_id: &Uuid,
        dkg_share_data: SignerDkgShareIdData,
    ) -> Result<(), DbError>;
}

#[async_trait]
pub trait SignerSignSessionStorage: Send + Sync + StorageHealthcheck {
    async fn get_sign_data(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
    ) -> Result<Option<SignerSignData>, DbError>;
    async fn set_sign_data(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
        sign_session_data: SignerSignData,
    ) -> Result<(), DbError>;
}
