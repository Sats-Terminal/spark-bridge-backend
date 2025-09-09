use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use persistent_storage::error::DatabaseError;
use tokio::sync::Mutex;

use crate::{errors::AggregatorError, signer::FrostSigner, traits::*, types::*};
use uuid::Uuid;

pub struct MockSignerMusigIdStorage {
    storage: Arc<Mutex<BTreeMap<MusigId, SignerMusigIdData>>>,
}

pub struct MockSignerSignSessionStorage {
    storage: Arc<Mutex<BTreeMap<(MusigId, Uuid), SignerSignData>>>,
}

impl MockSignerSignSessionStorage {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub async fn has_session(&self, musig_id: &MusigId, session_id: &Uuid) -> bool {
        let map = self.storage.lock().await;
        map.contains_key(&(musig_id.clone(), session_id.clone()))
    }
}

#[async_trait]
impl SignerSignSessionStorage for MockSignerSignSessionStorage {
    async fn get_sign_data(
        &self,
        musig_id: MusigId,
        session_id: Uuid,
    ) -> Result<Option<SignerSignData>, DatabaseError> {
        Ok(self
            .storage
            .lock()
            .await
            .get(&(musig_id.clone(), session_id.clone()))
            .cloned())
    }

    async fn set_sign_data(
        &self,
        musig_id: MusigId,
        session_id: Uuid,
        sign_session_data: SignerSignData,
    ) -> Result<(), DatabaseError> {
        self.storage
            .lock()
            .await
            .insert((musig_id.clone(), session_id.clone()), sign_session_data);
        Ok(())
    }
}

impl MockSignerMusigIdStorage {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

#[async_trait]
impl SignerMusigIdStorage for MockSignerMusigIdStorage {
    async fn get_musig_id_data(&self, musig_id: MusigId) -> Result<Option<SignerMusigIdData>, DatabaseError> {
        Ok(self
            .storage
            .lock()
            .await
            .get(&musig_id)
            .map(|musig_id_data| musig_id_data.clone()))
    }

    async fn set_musig_id_data(
        &self,
        musig_id: MusigId,
        musig_id_data: SignerMusigIdData,
    ) -> Result<(), DatabaseError> {
        self.storage.lock().await.insert(musig_id, musig_id_data);
        Ok(())
    }
}

pub struct MockAggregatorMusigIdStorage {
    storage: Arc<Mutex<BTreeMap<MusigId, AggregatorMusigIdData>>>,
}

pub struct MockAggregatorSignSessionStorage {
    storage: Arc<Mutex<BTreeMap<(MusigId, Uuid), AggregatorSignData>>>,
}

impl MockAggregatorMusigIdStorage {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

impl MockAggregatorSignSessionStorage {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

#[async_trait]
impl AggregatorMusigIdStorage for MockAggregatorMusigIdStorage {
    async fn get_musig_id_data(&self, musig_id: MusigId) -> Result<Option<AggregatorMusigIdData>, DatabaseError> {
        Ok(self
            .storage
            .lock()
            .await
            .get(&musig_id)
            .map(|musig_id_data| musig_id_data.clone()))
    }

    async fn set_musig_id_data(
        &self,
        musig_id: MusigId,
        musig_id_data: AggregatorMusigIdData,
    ) -> Result<(), DatabaseError> {
        self.storage.lock().await.insert(musig_id, musig_id_data);
        Ok(())
    }
}

#[async_trait]
impl AggregatorSignSessionStorage for MockAggregatorSignSessionStorage {
    async fn get_sign_data(
        &self,
        musig_id: MusigId,
        session_id: Uuid,
    ) -> Result<Option<AggregatorSignData>, DatabaseError> {
        Ok(self
            .storage
            .lock()
            .await
            .get(&(musig_id, session_id))
            .map(|sign_session_data| sign_session_data.clone()))
    }

    async fn set_sign_data(
        &self,
        musig_id: MusigId,
        session_id: Uuid,
        sign_session_data: AggregatorSignData,
    ) -> Result<(), DatabaseError> {
        self.storage
            .lock()
            .await
            .insert((musig_id, session_id), sign_session_data);
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
