use crate::errors::AggregatorError;
use crate::types::*;
use async_trait::async_trait;
use persistent_storage::error::DbError;
use std::fmt::Debug;
use uuid::Uuid;

#[async_trait]
pub trait SignerClient: Send + Sync + Debug {
    async fn dkg_round_1(&self, request: DkgRound1Request) -> Result<DkgRound1Response, AggregatorError>;

    async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response, AggregatorError>;

    async fn dkg_finalize(&self, request: DkgFinalizeRequest) -> Result<DkgFinalizeResponse, AggregatorError>;

    async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response, AggregatorError>;

    async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response, AggregatorError>;
}

#[async_trait]
pub trait AggregatorMusigIdStorage: Send + Sync + Debug {
    async fn get_musig_id_data(&self, musig_id: &MusigId) -> Result<Option<AggregatorMusigIdData>, DbError>;
    async fn set_musig_id_data(&self, musig_id: &MusigId, musig_id_data: AggregatorMusigIdData) -> Result<(), DbError>;
    async fn get_issuer_musig_id(&self, rune_id: String) -> Result<Option<MusigId>, DbError>;
}

#[async_trait]
pub trait AggregatorSignSessionStorage: Send + Sync + Debug {
    async fn get_sign_data(&self, musig_id: &MusigId, session_id: Uuid) -> Result<Option<AggregatorSignData>, DbError>;
    async fn set_sign_data(
        &self,
        musig_id: &MusigId,
        session_id: Uuid,
        sign_session_data: AggregatorSignData,
    ) -> Result<(), DbError>;
}

#[async_trait]
pub trait SignerMusigIdStorage: Send + Sync + Debug {
    async fn get_musig_id_data(&self, musig_id: &MusigId) -> Result<Option<SignerMusigIdData>, DbError>;
    async fn set_musig_id_data(&self, musig_id: &MusigId, musig_id_data: SignerMusigIdData) -> Result<(), DbError>;
}

#[async_trait]
pub trait SignerSignSessionStorage: Send + Sync + Debug {
    async fn get_sign_data(&self, musig_id: &MusigId, session_id: Uuid) -> Result<Option<SignerSignData>, DbError>;
    async fn set_sign_data(
        &self,
        musig_id: &MusigId,
        session_id: Uuid,
        sign_session_data: SignerSignData,
    ) -> Result<(), DbError>;
}
