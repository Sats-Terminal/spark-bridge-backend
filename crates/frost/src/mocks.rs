use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use persistent_storage::error::DbError;
use tokio::sync::Mutex;

use crate::{errors::AggregatorError, signer::FrostSigner, traits::*, types::*};
use persistent_storage::init::StorageHealthcheck;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MockSignerDkgShareIdStorage {
    storage: Arc<Mutex<BTreeMap<Uuid, SignerDkgShareIdData>>>,
}

#[derive(Default, Debug, Clone)]
pub struct MockSignerSignSessionStorage {
    storage: Arc<Mutex<BTreeMap<(Uuid, Uuid), SignerSignData>>>,
}

impl MockSignerSignSessionStorage {
    pub async fn has_session(&self, dkg_share_id: &Uuid, session_id: &Uuid) -> bool {
        let map = self.storage.lock().await;
        map.contains_key(&(*dkg_share_id, *session_id))
    }
}

#[async_trait]
impl StorageHealthcheck for MockSignerSignSessionStorage {
    async fn healthcheck(&self) -> Result<(), DbError> {
        Ok(())
    }
}

#[async_trait]
impl SignerSignSessionStorage for MockSignerSignSessionStorage {
    async fn get_sign_data(&self, dkg_share_id: &Uuid, session_id: Uuid) -> Result<Option<SignerSignData>, DbError> {
        Ok(self.storage.lock().await.get(&(*dkg_share_id, session_id)).cloned())
    }

    async fn set_sign_data(
        &self,
        dkg_share_data: &Uuid,
        session_id: Uuid,
        sign_session_data: SignerSignData,
    ) -> Result<(), DbError> {
        self.storage
            .lock()
            .await
            .insert((*dkg_share_data, session_id), sign_session_data);
        Ok(())
    }
}

impl Default for MockSignerDkgShareIdStorage {
    fn default() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

#[async_trait]
impl StorageHealthcheck for MockSignerDkgShareIdStorage {
    async fn healthcheck(&self) -> Result<(), DbError> {
        Ok(())
    }
}

#[async_trait]
impl SignerDkgShareStorage for MockSignerDkgShareIdStorage {
    async fn get_dkg_share_signer_data(&self, dkg_share_id: &Uuid) -> Result<Option<SignerDkgShareIdData>, DbError> {
        Ok(self.storage.lock().await.get(dkg_share_id).cloned())
    }

    async fn set_dkg_share_signer_data(
        &self,
        dkg_share_id: &Uuid,
        dkkg_share_data: SignerDkgShareIdData,
    ) -> Result<(), DbError> {
        self.storage.lock().await.insert(*dkg_share_id, dkkg_share_data);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MockAggregatorDkgShareIdStorage {
    storage: Arc<Mutex<BTreeMap<Uuid, AggregatorDkgShareData>>>,
}

impl Default for MockAggregatorDkgShareIdStorage {
    fn default() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::default())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MockAggregatorSignSessionStorage {
    storage: Arc<Mutex<BTreeMap<(Uuid, Uuid), AggregatorSignData>>>,
}

impl Default for MockAggregatorSignSessionStorage {
    fn default() -> Self {
        Self {
            storage: Arc::new(Mutex::new(BTreeMap::default())),
        }
    }
}

#[async_trait]
impl StorageHealthcheck for MockAggregatorDkgShareIdStorage {
    async fn healthcheck(&self) -> Result<(), DbError> {
        Ok(())
    }
}

#[async_trait]
impl AggregatorDkgShareStorage for MockAggregatorDkgShareIdStorage {
    async fn get_dkg_share_agg_data(&self, musig_id: &Uuid) -> Result<Option<AggregatorDkgShareData>, DbError> {
        Ok(self.storage.lock().await.get(musig_id).cloned())
    }

    async fn set_dkg_share_agg_data(
        &self,
        musig_id: &Uuid,
        dkg_share_data: AggregatorDkgShareData,
    ) -> Result<(), DbError> {
        self.storage.lock().await.insert(*musig_id, dkg_share_data);
        Ok(())
    }
}

#[async_trait]
impl StorageHealthcheck for MockAggregatorSignSessionStorage {
    async fn healthcheck(&self) -> Result<(), DbError> {
        Ok(())
    }
}

#[async_trait]
impl AggregatorSignSessionStorage for MockAggregatorSignSessionStorage {
    async fn get_sign_data(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
    ) -> Result<Option<AggregatorSignData>, DbError> {
        Ok(self.storage.lock().await.get(&(*dkg_share_id, session_id)).cloned())
    }

    async fn set_sign_data(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
        sign_session_data: AggregatorSignData,
    ) -> Result<(), DbError> {
        self.storage
            .lock()
            .await
            .insert((*dkg_share_id, session_id), sign_session_data);
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
