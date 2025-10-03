use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use persistent_storage::error::DbError;
use tokio::sync::Mutex;

use crate::{errors::AggregatorError, signer::FrostSigner, traits::*, types::*};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MockSignerMusigIdStorage {
    storage: Arc<Mutex<BTreeMap<MusigId, SignerMusigIdData>>>,
}

#[derive(Default, Debug, Clone)]
pub struct MockSignerSignSessionStorage {
    storage: Arc<Mutex<BTreeMap<(MusigId, Uuid), SignerSignData>>>,
}

impl MockSignerSignSessionStorage {
    pub async fn has_session(&self, musig_id: &MusigId, session_id: &Uuid) -> bool {
        let map = self.storage.lock().await;
        map.contains_key(&(musig_id.clone(), *session_id))
    }
}

#[async_trait]
impl SignerSignSessionStorage for MockSignerSignSessionStorage {
    async fn get_sign_data(&self, musig_id: &MusigId, session_id: Uuid) -> Result<Option<SignerSignData>, DbError> {
        Ok(self.storage.lock().await.get(&(musig_id.clone(), session_id)).cloned())
    }

    async fn set_sign_data(
        &self,
        musig_id: &MusigId,
        session_id: Uuid,
        sign_session_data: SignerSignData,
    ) -> Result<(), DbError> {
        self.storage
            .lock()
            .await
            .insert((musig_id.clone(), session_id), sign_session_data);
        Ok(())
    }
}

impl Default for MockSignerMusigIdStorage {
    fn default() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

#[async_trait]
impl SignerMusigIdStorage for MockSignerMusigIdStorage {
    async fn get_musig_id_data(&self, musig_id: &MusigId) -> Result<Option<SignerMusigIdData>, DbError> {
        Ok(self.storage.lock().await.get(musig_id).cloned())
    }

    async fn set_musig_id_data(&self, musig_id: &MusigId, musig_id_data: SignerMusigIdData) -> Result<(), DbError> {
        self.storage.lock().await.insert(musig_id.clone(), musig_id_data);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MockAggregatorMusigIdStorage {
    storage: Arc<Mutex<BTreeMap<MusigId, AggregatorMusigIdData>>>,
}

#[derive(Debug, Clone)]
pub struct MockAggregatorSignSessionStorage {
    storage: Arc<Mutex<BTreeMap<(MusigId, Uuid), AggregatorSignData>>>,
}

impl Default for MockAggregatorMusigIdStorage {
    fn default() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::default())),
        }
    }
}

impl Default for MockAggregatorSignSessionStorage {
    fn default() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::default())),
        }
    }
}

#[async_trait]
impl AggregatorMusigIdStorage for MockAggregatorMusigIdStorage {
    async fn get_musig_id_data(&self, musig_id: &MusigId) -> Result<Option<AggregatorMusigIdData>, DbError> {
        Ok(self.storage.lock().await.get(musig_id).cloned())
    }

    async fn set_musig_id_data(&self, musig_id: &MusigId, musig_id_data: AggregatorMusigIdData) -> Result<(), DbError> {
        self.storage.lock().await.insert(musig_id.clone(), musig_id_data);
        Ok(())
    }

    async fn get_issuer_musig_id(&self, _rune_id: String) -> Result<Option<MusigId>, DbError> {
        Ok(None)
    }
}

#[async_trait]
impl AggregatorSignSessionStorage for MockAggregatorSignSessionStorage {
    async fn get_sign_data(&self, musig_id: &MusigId, session_id: Uuid) -> Result<Option<AggregatorSignData>, DbError> {
        Ok(self.storage.lock().await.get(&(musig_id.clone(), session_id)).cloned())
    }

    async fn set_sign_data(
        &self,
        musig_id: &MusigId,
        session_id: Uuid,
        sign_session_data: AggregatorSignData,
    ) -> Result<(), DbError> {
        self.storage
            .lock()
            .await
            .insert((musig_id.clone(), session_id), sign_session_data);
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
            .map_err(AggregatorError::SignerError)
    }

    async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response, AggregatorError> {
        self.signer
            .dkg_round_2(request)
            .await
            .map_err(AggregatorError::SignerError)
    }

    async fn dkg_finalize(&self, request: DkgFinalizeRequest) -> Result<DkgFinalizeResponse, AggregatorError> {
        self.signer
            .dkg_finalize(request)
            .await
            .map_err(AggregatorError::SignerError)
    }

    async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response, AggregatorError> {
        self.signer
            .sign_round_1(request)
            .await
            .map_err(AggregatorError::SignerError)
    }

    async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response, AggregatorError> {
        self.signer
            .sign_round_2(request)
            .await
            .map_err(AggregatorError::SignerError)
    }
}
