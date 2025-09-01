use crate::traits::*;
use crate::errors::{SignerError, AggregatorError};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;
use crate::signer::FrostSigner;
use frost_secp256k1_tr::Identifier;

pub struct MockSignerUserStorage {
    user_states: Arc<Mutex<BTreeMap<String, SignerUserState>>>,
}

impl MockSignerUserStorage {
    pub fn new() -> Self {
        Self {
            user_states: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

#[async_trait]
impl SignerUserStorage for MockSignerUserStorage {
    async fn get_user_state(&self, user_id: String) -> Result<Option<SignerUserState>, SignerError> {
        Ok(self.user_states.lock().await.get(&user_id).map(|state| state.clone()))
    }

    async fn set_user_state(&self, user_id: String, state: SignerUserState) -> Result<(), SignerError> {
        self.user_states.lock().await.insert(user_id, state);
        Ok(())
    }
}

pub struct MockAggregatorUserStorage {
    user_states: Arc<Mutex<BTreeMap<String, AggregatorUserState>>>,
}

impl MockAggregatorUserStorage {
    pub fn new() -> Self {
        Self {
            user_states: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

#[async_trait]
impl AggregatorUserStorage for MockAggregatorUserStorage {
    async fn get_user_state(&self, user_id: String) -> Result<Option<AggregatorUserState>, AggregatorError> {
        Ok(self.user_states.lock().await.get(&user_id).map(|state| state.clone()))
    }

    async fn set_user_state(&self, user_id: String, state: AggregatorUserState) -> Result<(), AggregatorError> {
        self.user_states.lock().await.insert(user_id, state);
        Ok(())
    }
}

#[derive(Clone)]
pub struct MockSignerClient {
    signer: FrostSigner,
}

impl MockSignerClient {
    pub fn new(signer: FrostSigner) -> Self {
        Self { signer }
    }
}

#[async_trait]
impl SignerClient for MockSignerClient {
    async fn dkg_round_1(&self, request: DkgRound1Request) -> Result<DkgRound1Response, AggregatorError> {
        self.signer.dkg_round_1(request).await.map_err(|e| AggregatorError::SignerError(e))
    }

    async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response, AggregatorError> {
        self.signer.dkg_round_2(request).await.map_err(|e| AggregatorError::SignerError(e))
    }

    async fn dkg_finalize(&self, request: DkgFinalizeRequest) -> Result<DkgFinalizeResponse, AggregatorError> {
        self.signer.dkg_finalize(request).await.map_err(|e| AggregatorError::SignerError(e))
    }

    async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response, AggregatorError> {
        self.signer.sign_round_1(request).await.map_err(|e| AggregatorError::SignerError(e))
    }

    async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response, AggregatorError> {
        self.signer.sign_round_2(request).await.map_err(|e| AggregatorError::SignerError(e))
    }
}

