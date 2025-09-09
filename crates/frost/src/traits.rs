use crate::errors::AggregatorError;
use crate::types::*;
use async_trait::async_trait;
use persistent_storage::error::DatabaseError;
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
pub trait AggregatorMusigIdStorage: Send + Sync {
    async fn get_musig_id_data(&self, musig_id: MusigId) -> Result<Option<AggregatorMusigIdData>, DatabaseError>;
    async fn set_musig_id_data(
        &self,
        musig_id: MusigId,
        musig_id_data: AggregatorMusigIdData,
    ) -> Result<(), DatabaseError>;
}

#[async_trait]
pub trait AggregatorSignSessionStorage: Send + Sync {
    async fn get_sign_data(
        &self,
        musig_id: MusigId,
        session_id: Uuid,
    ) -> Result<Option<AggregatorSignData>, DatabaseError>;
    async fn set_sign_data(
        &self,
        musig_id: MusigId,
        session_id: Uuid,
        sign_session_data: AggregatorSignData,
    ) -> Result<(), DatabaseError>;
}

#[async_trait]
pub trait SignerMusigIdStorage: Send + Sync {
    async fn get_musig_id_data(&self, musig_id: MusigId) -> Result<Option<SignerMusigIdData>, DatabaseError>;
    async fn set_musig_id_data(&self, musig_id: MusigId, musig_id_data: SignerMusigIdData)
    -> Result<(), DatabaseError>;
}

#[async_trait]
pub trait SignerSignSessionStorage: Send + Sync {
    async fn get_sign_data(&self, musig_id: MusigId, session_id: Uuid)
    -> Result<Option<SignerSignData>, DatabaseError>;
    async fn set_sign_data(
        &self,
        musig_id: MusigId,
        session_id: Uuid,
        sign_session_data: SignerSignData,
    ) -> Result<(), DatabaseError>;
}
