use persistent_storage::error::DatabaseError;
use async_trait::async_trait;
use bitcoin::secp256k1::PublicKey;
use uuid::Uuid;
use crate::errors::{AggregatorError, SignerError};
use crate::types::*;

#[async_trait]
pub trait SignerClient: Send + Sync {
    async fn dkg_round_1(&self, request: DkgRound1Request) -> Result<DkgRound1Response, AggregatorError>;

    async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response, AggregatorError>;

    async fn dkg_finalize(&self, request: DkgFinalizeRequest) -> Result<DkgFinalizeResponse, AggregatorError>;

    async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response, AggregatorError>;

    async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response, AggregatorError>;
}

#[async_trait]
pub trait AggregatorUserKeyStorage: Send + Sync {
    async fn get_key_info(&self, user_public_key: PublicKey) -> Result<Option<AggregatorUserKeyInfo>, DatabaseError>;
    async fn set_key_info(&self, user_public_key: PublicKey, user_key_info: AggregatorUserKeyInfo) -> Result<(), DatabaseError>;
}

#[async_trait]
pub trait AggregatorUserSessionStorage: Send + Sync {
    async fn get_session_info(&self, user_public_key: PublicKey, session_id: Uuid) -> Result<Option<AggregatorUserSessionInfo>, DatabaseError>;
    async fn set_session_info(&self, user_public_key: PublicKey, session_id: Uuid, user_session_info: AggregatorUserSessionInfo) -> Result<(), DatabaseError>;
}

#[async_trait]
pub trait SignerUserKeyStorage: Send + Sync {
    async fn get_key_info(&self, user_public_key: PublicKey) -> Result<Option<SignerUserKeyInfo>, DatabaseError>;
    async fn set_key_info(&self, user_public_key: PublicKey, user_key_info: SignerUserKeyInfo) -> Result<(), DatabaseError>;
}

#[async_trait]
pub trait SignerUserSessionStorage: Send + Sync {
    async fn get_session_info(&self, user_public_key: PublicKey, session_id: Uuid) -> Result<Option<SignerUserSessionInfo>, DatabaseError>;
    async fn set_session_info(&self, user_public_key: PublicKey, session_id: Uuid, user_session_info: SignerUserSessionInfo) -> Result<(), DatabaseError>;
}
