use crate::types::Nonce;
use crate::{errors::AggregatorError, traits::*, types::*};
use frost_secp256k1_tr::{Identifier, Signature, SigningPackage, keys, keys::Tweak};
use futures::future::join_all;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use tokio::sync::Mutex;
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct FrostAggregator {
    verifiers: BTreeMap<Identifier, Arc<dyn SignerClient>>,
    dkg_share_storage: Arc<dyn AggregatorDkgShareStorage>,
    sign_session_storage: Arc<dyn AggregatorSignSessionStorage>,
    locked_dkg_share_ids: Arc<Mutex<BTreeSet<DkgShareId>>>,
}

impl FrostAggregator {
    pub fn new(
        verifiers: BTreeMap<Identifier, Arc<dyn SignerClient>>,
        musig_id_storage: Arc<dyn AggregatorDkgShareStorage>,
        sign_session_storage: Arc<dyn AggregatorSignSessionStorage>,
    ) -> Self {
        Self {
            verifiers,
            dkg_share_storage: musig_id_storage,
            sign_session_storage,
            locked_dkg_share_ids: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub fn into_shared(self) -> Arc<Self> {
        Arc::new(self)
    }

    #[instrument]
    async fn dkg_round_1(&self, dkg_share_id: &DkgShareId) -> Result<(), AggregatorError> {
        let dkg_share_data = self.dkg_share_storage.get_dkg_share_agg_data(dkg_share_id).await?;

        match dkg_share_data {
            None => Err(AggregatorError::InvalidUserState("User DkgState is None".to_string())),
            Some(AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::Initialized,
            }) => {
                let signer_clients_request = DkgRound1Request {
                    dkg_share_id: *dkg_share_id,
                };

                let mut verifier_responses = BTreeMap::new();
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let verifier_signer_clients_request = signer_clients_request.clone();
                    let join_handle = async move {
                        (
                            verifier_id,
                            signer_client.dkg_round_1(verifier_signer_clients_request).await,
                        )
                    };
                    join_handles.push(join_handle);
                }

                let join_handles = join_all(join_handles).await;

                for (verifier_id, response) in join_handles {
                    verifier_responses.insert(verifier_id, response?.round1_package);
                }

                self.dkg_share_storage
                    .set_dkg_share_agg_data(
                        dkg_share_id,
                        AggregatorDkgShareData {
                            dkg_state: AggregatorDkgState::DkgRound1 {
                                round1_packages: verifier_responses,
                            },
                        },
                    )
                    .await?;

                Ok(())
            }
            Some(x) => Err(AggregatorError::InvalidUserState(format!(
                "User state is {:?}",
                x.dkg_state
            ))),
        }
    }

    #[instrument]
    async fn dkg_round_2(&self, dkg_share_id: &DkgShareId) -> Result<(), AggregatorError> {
        let dkg_share_data = self.dkg_share_storage.get_dkg_share_agg_data(dkg_share_id).await?;

        match dkg_share_data {
            Some(AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::DkgRound1 { round1_packages },
            }) => {
                let mut verifier_responses = BTreeMap::new();
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let mut packages = round1_packages.clone();
                    packages.remove(&verifier_id);
                    let signer_requests = DkgRound2Request {
                        dkg_share_id: *dkg_share_id,
                        round1_packages: packages,
                    };
                    let join_handle = async move { (verifier_id, signer_client.dkg_round_2(signer_requests).await) };
                    join_handles.push(join_handle);
                }

                let join_handles = join_all(join_handles).await;

                for (verifier_id, response) in join_handles {
                    for (receiver_identifier, round2_package) in response?.round2_packages {
                        verifier_responses
                            .entry(receiver_identifier)
                            .or_insert(BTreeMap::new())
                            .insert(verifier_id, round2_package);
                    }
                }

                self.dkg_share_storage
                    .set_dkg_share_agg_data(
                        dkg_share_id,
                        AggregatorDkgShareData {
                            dkg_state: AggregatorDkgState::DkgRound2 {
                                round1_packages,
                                round2_packages: verifier_responses,
                            },
                        },
                    )
                    .await?;
                Ok(())
            }
            _ => Err(AggregatorError::InvalidUserState(
                "User key DkgState is not DkgRound2".to_string(),
            )),
        }
    }

    #[instrument]
    async fn dkg_finalize(&self, dkg_share_id: &DkgShareId) -> Result<(), AggregatorError> {
        let dkg_share_data = self.dkg_share_storage.get_dkg_share_agg_data(dkg_share_id).await?;

        match dkg_share_data {
            Some(AggregatorDkgShareData {
                dkg_state:
                    AggregatorDkgState::DkgRound2 {
                        round1_packages,
                        round2_packages,
                    },
            }) => {
                let mut public_key_packages = vec![];
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let mut verifier_round1_packages = round1_packages.clone();
                    verifier_round1_packages.remove(&verifier_id);
                    let request = DkgFinalizeRequest {
                        dkg_share_id: *dkg_share_id,
                        round1_packages: verifier_round1_packages,
                        round2_packages: round2_packages
                            .get(&verifier_id)
                            .ok_or(AggregatorError::Internal("Round2 packages not found".to_string()))?
                            .clone(),
                    };
                    let join_handle = async move { (verifier_id, signer_client.dkg_finalize(request).await) };
                    join_handles.push(join_handle);
                }

                let join_handles = join_all(join_handles).await;

                for (_verifier_id, response) in join_handles {
                    let public_key_package = response?.public_key_package;
                    public_key_packages.push(public_key_package);
                }

                let public_key_package = public_key_packages[0].clone();
                for _public_key_package in public_key_packages {
                    if public_key_package != _public_key_package {
                        return Err(AggregatorError::Internal(
                            "Public key packages are not equal".to_string(),
                        ));
                    }
                }

                self.dkg_share_storage
                    .set_dkg_share_agg_data(
                        dkg_share_id,
                        AggregatorDkgShareData {
                            dkg_state: AggregatorDkgState::DkgFinalized {
                                public_key_package: public_key_package.clone(),
                            },
                        },
                    )
                    .await?;

                Ok(())
            }
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }

    #[instrument]
    pub async fn lock_dkg_share(&self, dkg_share_id: &DkgShareId) -> Result<(), AggregatorError> {
        let mut locked_dkg_shares = self.locked_dkg_share_ids.lock().await;
        if locked_dkg_shares.contains(dkg_share_id) {
            return Err(AggregatorError::DkgShareIdAlreadyExists(format!(
                "Dkg share id already exists: {:?}",
                dkg_share_id
            )));
        }
        locked_dkg_shares.insert(*dkg_share_id);
        Ok(())
    }

    #[instrument]
    pub async fn unlock_dkg_share_id(&self, dkg_share_id: &DkgShareId) -> Result<(), AggregatorError> {
        let mut locked_dkg_share_ids = self.locked_dkg_share_ids.lock().await;
        let removed = locked_dkg_share_ids.remove(dkg_share_id);
        if !removed {
            return Err(AggregatorError::DkgShareIdNotFound(format!(
                "Something bad went wrong: {:?}",
                dkg_share_id
            )));
        }
        Ok(())
    }

    #[instrument]
    pub async fn run_dkg_flow(&self, dkg_share_id: &DkgShareId) -> Result<keys::PublicKeyPackage, AggregatorError> {
        self.lock_dkg_share(dkg_share_id).await?;

        let dkg_share_data = self.dkg_share_storage.get_dkg_share_agg_data(dkg_share_id).await?;
        if let Some(x) = dkg_share_data.as_ref() {
            match &x.dkg_state {
                AggregatorDkgState::Initialized => {}
                _ => {
                    self.unlock_dkg_share_id(dkg_share_id).await?;
                    return Err(AggregatorError::DkgShareIdAlreadyExists(format!(
                        "Dkg share id already exists: {:?}",
                        dkg_share_id
                    )));
                }
            }
        }

        self.dkg_round_1(dkg_share_id).await?;
        self.dkg_round_2(dkg_share_id).await?;
        self.dkg_finalize(dkg_share_id).await?;

        let dkg_share_data = self.dkg_share_storage.get_dkg_share_agg_data(dkg_share_id).await?;
        match dkg_share_data {
            Some(AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::DkgFinalized { public_key_package },
            }) => {
                self.unlock_dkg_share_id(dkg_share_id).await?;
                Ok(public_key_package)
            }
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }

    #[instrument]
    async fn sign_round_1(
        &self,
        dkg_share_id: &DkgShareId,
        session_id: Uuid,
        message_hash: &[u8],
        metadata: SigningMetadata,
        tweak: Option<Nonce>,
    ) -> Result<(), AggregatorError> {
        let dkg_share_data = self.dkg_share_storage.get_dkg_share_agg_data(dkg_share_id).await?;

        match dkg_share_data {
            Some(AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::DkgFinalized { public_key_package },
            }) => {
                let mut commitments = BTreeMap::new();
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let request = SignRound1Request {
                        dkg_share_id: *dkg_share_id,
                        metadata: metadata.clone(),
                        message_hash: message_hash.to_vec(),
                        session_id,
                        tweak: tweak.map(|t| t.to_vec()),
                    };
                    let join_handle = async move { (verifier_id, signer_client.sign_round_1(request).await) };
                    join_handles.push(join_handle);
                }

                let join_handles = join_all(join_handles).await;

                for (verifier_id, response) in join_handles {
                    commitments.insert(verifier_id, response?.commitments);
                }

                let signing_package = SigningPackage::new(commitments.clone(), message_hash);

                self.sign_session_storage
                    .set_sign_data(
                        dkg_share_id,
                        session_id,
                        AggregatorSignData {
                            tweak: tweak.map(|t| t.to_vec()),
                            message_hash: message_hash.to_vec(),
                            metadata,
                            sign_state: AggregatorSignState::SigningRound1 { signing_package },
                        },
                    )
                    .await?;

                Ok(())
            }
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }

    #[instrument]
    async fn sign_round_2(&self, dkg_share_id: &DkgShareId, session_id: Uuid) -> Result<(), AggregatorError> {
        let dkg_share_data = self.dkg_share_storage.get_dkg_share_agg_data(dkg_share_id).await?;
        let mut sign_data = self
            .sign_session_storage
            .get_sign_data(dkg_share_id, session_id)
            .await?
            .ok_or(AggregatorError::InvalidUserState(
                "Session state is not SigningRound1".to_string(),
            ))?;

        let public_key_package = match dkg_share_data {
            Some(AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::DkgFinalized { public_key_package },
            }) => public_key_package,
            _ => {
                return Err(AggregatorError::InvalidUserState(
                    "User state is not DkgFinalized".to_string(),
                ));
            }
        };

        match sign_data.clone() {
            AggregatorSignData {
                tweak,
                message_hash,
                metadata: _,
                sign_state: AggregatorSignState::SigningRound1 { signing_package },
            } => {
                let tweaked_public_key_package = match tweak.clone() {
                    Some(tweak) => public_key_package.clone().tweak(Some(tweak.to_vec())),
                    None => public_key_package.clone(),
                };
                let mut signature_shares = BTreeMap::new();
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let request = SignRound2Request {
                        dkg_share_id: *dkg_share_id,
                        session_id,
                        signing_package: signing_package.clone(),
                    };
                    let join_handle = async move { (verifier_id, signer_client.sign_round_2(request).await) };
                    join_handles.push(join_handle);
                }

                let join_handles = join_all(join_handles).await;

                for (verifier_id, response) in join_handles {
                    signature_shares.insert(verifier_id, response?.signature_share);
                }

                let signature =
                    frost_secp256k1_tr::aggregate(&signing_package, &signature_shares, &tweaked_public_key_package)
                        .map_err(|e| AggregatorError::Internal(format!("Signature aggregation failed: {:?}", e)))?;

                let is_valid = tweaked_public_key_package
                    .verifying_key()
                    .verify(message_hash.as_slice(), &signature)
                    .is_ok();
                if !is_valid {
                    return Err(AggregatorError::Internal("Signature is not valid".to_string()));
                }

                sign_data.sign_state = AggregatorSignState::SigningRound2 { signature };
                self.sign_session_storage
                    .set_sign_data(dkg_share_id, session_id, sign_data)
                    .await?;

                Ok(())
            }
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }

    #[instrument]
    pub async fn run_signing_flow(
        &self,
        dkg_share_id: DkgShareId,
        message_hash: &[u8],
        metadata: SigningMetadata,
        tweak: Option<Nonce>,
    ) -> Result<Signature, AggregatorError> {
        let session_id = global_utils::common_types::get_uuid();

        self.sign_round_1(&dkg_share_id, session_id, message_hash, metadata, tweak)
            .await?;
        self.sign_round_2(&dkg_share_id, session_id).await?;

        let sign_data = self
            .sign_session_storage
            .get_sign_data(&dkg_share_id, session_id)
            .await?;
        let state = sign_data
            .ok_or(AggregatorError::InvalidUserState(
                "Session state is not SigningRound2".to_string(),
            ))?
            .sign_state;

        match state {
            AggregatorSignState::SigningRound2 { signature } => Ok(signature),
            _ => Err(AggregatorError::InvalidUserState(
                "Session state is not SigningRound2".to_string(),
            )),
        }
    }

    pub async fn get_public_key_package(
        &self,
        dkg_share_id: DkgShareId,
        tweak: Option<Nonce>,
    ) -> Result<keys::PublicKeyPackage, AggregatorError> {
        let dkg_share_data = self.dkg_share_storage.get_dkg_share_agg_data(&dkg_share_id).await?;

        match dkg_share_data {
            Some(AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::DkgFinalized { public_key_package },
            }) => {
                let tweaked_public_key_package = match tweak {
                    Some(tweak) => public_key_package.clone().tweak(Some(tweak.to_vec())),
                    None => public_key_package.clone(),
                };
                Ok(tweaked_public_key_package)
            }
            None => Err(AggregatorError::InvalidUserState("User state is not found".to_string())),
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }
}
