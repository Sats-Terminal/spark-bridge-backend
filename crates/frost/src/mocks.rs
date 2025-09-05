use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use persistent_storage::error::DatabaseError;
use tokio::sync::Mutex;

use crate::{
    errors::{AggregatorError, SignerError},
    signer::FrostSigner,
    traits::*,
    types::*,
};
use bitcoin::secp256k1::PublicKey;
use uuid::Uuid;

pub struct MockSignerUserKeyStorage {
    storage: Arc<Mutex<BTreeMap<PublicKey, SignerUserKeyInfo>>>,
}

pub struct MockSignerSessionStorage {
    storage: Arc<Mutex<BTreeMap<(PublicKey, Uuid), SignerUserSessionInfo>>>,
}

impl MockSignerSessionStorage {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub async fn has_session(&self, user_public_key: &PublicKey, session_id: &Uuid) -> bool {
        let map = self.storage.lock().await;
        map.contains_key(&(*user_public_key, *session_id))
    }
}

#[async_trait]
impl SignerUserSessionStorage for MockSignerSessionStorage {
    async fn get_session_info(
        &self,
        user_public_key: PublicKey,
        session_id: Uuid,
    ) -> Result<Option<SignerUserSessionInfo>, DatabaseError> {
        Ok(self
            .storage
            .lock()
            .await
            .get(&(user_public_key.clone(), session_id.clone()))
            .cloned())
    }

    async fn set_session_info(
        &self,
        user_public_key: PublicKey,
        session_id: Uuid,
        session_info: SignerUserSessionInfo,
    ) -> Result<(), DatabaseError> {
        self.storage
            .lock()
            .await
            .insert((user_public_key.clone(), session_id.clone()), session_info);
        Ok(())
    }
}

impl MockSignerUserKeyStorage {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

#[async_trait]
impl SignerUserKeyStorage for MockSignerUserKeyStorage {
    async fn get_key_info(&self, user_public_key: PublicKey) -> Result<Option<SignerUserKeyInfo>, DatabaseError> {
        Ok(self.storage.lock().await.get(&user_public_key).map(|key_info| key_info.clone()))
    }

    async fn set_key_info(&self, user_public_key: PublicKey, key_info: SignerUserKeyInfo) -> Result<(), DatabaseError> {
        self.storage.lock().await.insert(user_public_key, key_info);
        Ok(())
    }
}

pub struct MockAggregatorUserKeyStorage {
    storage: Arc<Mutex<BTreeMap<PublicKey, AggregatorUserKeyInfo>>>,
}

pub struct MockAggregatorUserSessionStorage {
    storage: Arc<Mutex<BTreeMap<(PublicKey, Uuid), AggregatorUserSessionInfo>>>,
}

impl MockAggregatorUserKeyStorage {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

impl MockAggregatorUserSessionStorage {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

#[async_trait]
impl AggregatorUserKeyStorage for MockAggregatorUserKeyStorage {
    async fn get_key_info(&self, user_public_key: PublicKey) -> Result<Option<AggregatorUserKeyInfo>, DatabaseError> {
        Ok(self.storage.lock().await.get(&user_public_key).map(|key_info| key_info.clone()))
    }

    async fn set_key_info(&self, user_public_key: PublicKey, key_info: AggregatorUserKeyInfo) -> Result<(), DatabaseError> {
        self.storage.lock().await.insert(user_public_key, key_info);
        Ok(())
    }
}

#[async_trait]
impl AggregatorUserSessionStorage for MockAggregatorUserSessionStorage {
    async fn get_session_info(&self, user_public_key: PublicKey, session_id: Uuid) -> Result<Option<AggregatorUserSessionInfo>, DatabaseError> {
        Ok(self.storage.lock().await.get(&(user_public_key, session_id)).map(|session_info| session_info.clone()))
    }

    async fn set_session_info(&self, user_public_key: PublicKey, session_id: Uuid, session_info: AggregatorUserSessionInfo) -> Result<(), DatabaseError> {
        self.storage.lock().await.insert((user_public_key, session_id), session_info);
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
        self.signer
            .dkg_round_1(request)
            .await
            .map_err(|e| AggregatorError::SignerError(e))
    }

    async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response, AggregatorError> {
        self.signer
            .dkg_round_2(request)
            .await
            .map_err(|e| AggregatorError::SignerError(e))
    }

    async fn dkg_finalize(&self, request: DkgFinalizeRequest) -> Result<DkgFinalizeResponse, AggregatorError> {
        self.signer
            .dkg_finalize(request)
            .await
            .map_err(|e| AggregatorError::SignerError(e))
    }

    async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response, AggregatorError> {
        self.signer
            .sign_round_1(request)
            .await
            .map_err(|e| AggregatorError::SignerError(e))
    }

    async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response, AggregatorError> {
        self.signer
            .sign_round_2(request)
            .await
            .map_err(|e| AggregatorError::SignerError(e))
    }
}
