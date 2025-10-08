use crate::types::TweakBytes;
use crate::{errors::AggregatorError, traits::*, types::*};
use frost_secp256k1_tr::{Identifier, Signature, SigningPackage, keys, keys::Tweak};
use futures::future::join_all;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use tokio::sync::Mutex;
use tracing::debug;
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct FrostAggregator {
    verifiers: BTreeMap<Identifier, Arc<dyn SignerClient>>,
    dkg_share_storage: Arc<dyn AggregatorDkgShareStorage>,
    sign_session_storage: Arc<dyn AggregatorSignSessionStorage>,
    locked_dkg_share_ids: Arc<Mutex<BTreeSet<Uuid>>>,
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

    async fn dkg_round_1(&self, dkg_share_id: &Uuid) -> Result<(), AggregatorError> {
        debug!(dkg_share_id = ?dkg_share_id, "Starting DKG Round 1");
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

                debug!(dkg_share_id = ?dkg_share_id, verifiers_count = self.verifiers.len(), "DKG Round 1 completed");
                Ok(())
            }
            Some(x) => Err(AggregatorError::InvalidUserState(format!(
                "User state is {:?}",
                x.dkg_state
            ))),
        }
    }

    async fn dkg_round_2(&self, dkg_share_id: &Uuid) -> Result<(), AggregatorError> {
        debug!(dkg_share_id = ?dkg_share_id, "Starting DKG Round 2");
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

                debug!(dkg_share_id = ?dkg_share_id, verifiers_count = self.verifiers.len(), "DKG Round 2 completed");
                Ok(())
            }
            _ => Err(AggregatorError::InvalidUserState(
                "User key DkgState is not DkgRound2".to_string(),
            )),
        }
    }

    async fn dkg_finalize(&self, dkg_share_id: &Uuid) -> Result<(), AggregatorError> {
        debug!(dkg_share_id = ?dkg_share_id, "Starting DKG flow");
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

                debug!(dkg_share_id = ?dkg_share_id, "DKG flow completed successfully");
                Ok(())
            }
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }

    pub async fn run_dkg_flow(&self) -> Result<DkgResponse, AggregatorError> {

        let dkg_share_id = Uuid::new_v4();

        self.dkg_share_storage.set_dkg_share_agg_data(&dkg_share_id, AggregatorDkgShareData {
            dkg_state: AggregatorDkgState::Initialized,
        }).await?;

        self.dkg_round_1(&dkg_share_id).await?;
        self.dkg_round_2(&dkg_share_id).await?;
        self.dkg_finalize(&dkg_share_id).await?;

        let dkg_share_data = self.dkg_share_storage.get_dkg_share_agg_data(&dkg_share_id).await?;
        match dkg_share_data {
            Some(AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::DkgFinalized { public_key_package },
            }) => {
                Ok(DkgResponse {
                    dkg_share_id,
                    public_key_package,
                })
            }
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }

    async fn dkg_round_1_batch(&self, dkg_share_ids: Vec<Uuid>) -> Result<(), AggregatorError> {
        let dkg_shares_data = self.dkg_share_storage.get_batch_dkg_share_agg_data(dkg_share_ids.clone()).await?;
        for dkg_share_data in dkg_shares_data {
            match dkg_share_data {
                AggregatorDkgShareData { dkg_state: AggregatorDkgState::Initialized, } => {}
                _ => return Err(AggregatorError::InvalidUserState("User state is not Initialized".to_string())),
            }
        }

        let signer_clients_request = dkg_share_ids.iter().map(|dkg_share_id| DkgRound1Request {
            dkg_share_id: *dkg_share_id,
        }).collect::<Vec<DkgRound1Request>>();

        let mut verifier_responses = Vec::new();
        for _ in 0..dkg_share_ids.len() {
            verifier_responses.push(BTreeMap::new());
        }
        
        let mut join_handles = vec![];

        for (verifier_id, signer_client) in self.verifiers.clone() {
            let verifier_signer_clients_request = DkgRound1BatchRequest {
                dkg_round_1_requests: signer_clients_request.clone(),
            };
            let join_handle = async move { 
                (verifier_id, signer_client.dkg_batch_round_1(verifier_signer_clients_request).await) 
            };
            join_handles.push(join_handle);
        }

        let join_handles = join_all(join_handles).await;

        for (verifier_id, response) in join_handles {
            for (i, dkg_round_1_response) in response?.dkg_round_1_responses.into_iter().enumerate() {
                verifier_responses[i].insert(verifier_id, dkg_round_1_response.round1_package);
            }
        }

        let dkg_shares_ids_and_data = verifier_responses
            .into_iter()
            .zip(dkg_share_ids.into_iter())
            .map(|(verifier_response, dkg_share_id)| (dkg_share_id, AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::DkgRound1 {
                    round1_packages: verifier_response,
                },
            }))
            .collect::<Vec<(Uuid, AggregatorDkgShareData)>>();

        self.dkg_share_storage.set_batch_dkg_share_agg_data(dkg_shares_ids_and_data).await?;

        Ok(())
    }

    async fn dkg_round_2_batch(&self, dkg_share_ids: Vec<Uuid>) -> Result<(), AggregatorError> {
        let dkg_shares_data = self.dkg_share_storage.get_batch_dkg_share_agg_data(dkg_share_ids.clone()).await?;
        let mut round1_packages_vec = vec![];
        for dkg_share_data in dkg_shares_data {
            match dkg_share_data {
                AggregatorDkgShareData { dkg_state: AggregatorDkgState::DkgRound1 { round1_packages }, } => {
                    round1_packages_vec.push(round1_packages);
                }
                _ => return Err(AggregatorError::InvalidUserState("User state is not DkgRound1".to_string())),
            }
        }

        let mut verifier_responses = Vec::new();
        for _ in 0..dkg_share_ids.len() {
            verifier_responses.push(BTreeMap::new());
        }
        
        let mut join_handles = vec![];
        
        for (verifier_id, signer_client) in self.verifiers.clone() {
            let mut requests = vec![];
            for i in 0..dkg_share_ids.len() {
                let mut round1_packages = round1_packages_vec[i].clone();
                round1_packages.remove(&verifier_id);
                requests.push(DkgRound2Request {
                    dkg_share_id: dkg_share_ids[i],
                    round1_packages,
                });
            }
            let verifier_request = DkgRound2BatchRequest {
                dkg_round_2_requests: requests,
            };
            let join_handle = async move { (verifier_id, signer_client.dkg_batch_round_2(verifier_request).await) };
            join_handles.push(join_handle);
        }

        let join_handles = join_all(join_handles).await;

        for (verifier_id, response) in join_handles {
            for (i, dkg_round_2_response) in response?.dkg_round_2_responses.into_iter().enumerate() {
                for (receiver_identifier, round2_package) in dkg_round_2_response.round2_packages {
                    verifier_responses[i]
                        .entry(receiver_identifier)
                        .or_insert(BTreeMap::new())
                        .insert(verifier_id, round2_package);
                }
            }
        }

        let dkg_shares_ids_and_data = dkg_share_ids
            .into_iter()
            .zip(verifier_responses.into_iter())
            .zip(round1_packages_vec.into_iter())
            .map(|((dkg_share_id, verifier_response), round1_packages)| (dkg_share_id, AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::DkgRound2 {
                    round1_packages,
                    round2_packages: verifier_response,
                },
            }))
            .collect::<Vec<(Uuid, AggregatorDkgShareData)>>();

        self.dkg_share_storage.set_batch_dkg_share_agg_data(dkg_shares_ids_and_data).await?;

        Ok(())
    }

    async fn dkg_finalize_batch(&self, dkg_share_ids: Vec<Uuid>) -> Result<(), AggregatorError> {
        let dkg_shares_data = self.dkg_share_storage.get_batch_dkg_share_agg_data(dkg_share_ids.clone()).await?;

        let mut round1_packages_vec = vec![];
        let mut round2_packages_vec = vec![];
        for dkg_share_data in dkg_shares_data {
            match dkg_share_data {
                AggregatorDkgShareData { dkg_state: AggregatorDkgState::DkgRound2 { round1_packages, round2_packages }, } => {
                    round1_packages_vec.push(round1_packages);
                    round2_packages_vec.push(round2_packages);
                }
                _ => return Err(AggregatorError::InvalidUserState("User state is not DkgRound2".to_string())),
            }
        }

        let mut public_key_packages_vec = vec![];
        for _ in 0..self.verifiers.len() {
            public_key_packages_vec.push(vec![]);
        }

        let mut join_handles = vec![];

        for (verifier_id, signer_client) in self.verifiers.clone() {
            let mut requests = vec![];

            for i in 0..dkg_share_ids.len() {
                let mut verifier_round1_packages = round1_packages_vec[i].clone();
                verifier_round1_packages.remove(&verifier_id);
                let verifier_round2_packages = round2_packages_vec[i]
                    .get(&verifier_id)
                    .ok_or(AggregatorError::Internal("Round2 packages not found".to_string()))?
                    .clone();
                requests.push(DkgFinalizeRequest {
                    dkg_share_id: dkg_share_ids[i],
                    round1_packages: verifier_round1_packages,
                    round2_packages: verifier_round2_packages,
                });
            }

            let verifier_request = DkgFinalizeBatchRequest {
                dkg_finalize_requests: requests,
            };

            let join_handle = async move { (verifier_id, signer_client.dkg_batch_finalize(verifier_request).await) };
            join_handles.push(join_handle);
        }

        let join_handles = join_all(join_handles).await;

        for (i, (_verifier_id, response)) in join_handles.into_iter().enumerate() {
            for dkg_finalize_response in response?.dkg_finalize_responses.into_iter() {
                public_key_packages_vec[i].push(dkg_finalize_response.public_key_package);
            }
        }

        for i in 0..public_key_packages_vec[0].len() {
            for j in 1..self.verifiers.len() {
                if public_key_packages_vec[0][i] != public_key_packages_vec[j][i] {
                    return Err(AggregatorError::Internal("Public key packages are not equal".to_string()));
                }
            }
        }

        let dkg_shares_ids_and_data = dkg_share_ids
            .into_iter()
            .zip(public_key_packages_vec[0].clone().into_iter())
            .map(|(dkg_share_id, public_key_package)| (dkg_share_id, AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::DkgFinalized { public_key_package },
            }))
            .collect::<Vec<(Uuid, AggregatorDkgShareData)>>();

        self.dkg_share_storage.set_batch_dkg_share_agg_data(dkg_shares_ids_and_data).await?;

        Ok(())
    }

    pub async fn run_dkg_flow_batch(&self, n_shares: u32) -> Result<Vec<DkgResponse>, AggregatorError> {
        let dkg_share_ids = (0..n_shares).map(|_| Uuid::new_v4()).collect::<Vec<Uuid>>();

        let dkg_shares_ids_and_data = dkg_share_ids.clone()
            .into_iter()
            .map(|dkg_share_id| (dkg_share_id, AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::Initialized,
            }))
            .collect::<Vec<(Uuid, AggregatorDkgShareData)>>();
        
        self.dkg_share_storage.set_batch_dkg_share_agg_data(dkg_shares_ids_and_data).await?;

        self.dkg_round_1_batch(dkg_share_ids.clone()).await?;
        self.dkg_round_2_batch(dkg_share_ids.clone()).await?;
        self.dkg_finalize_batch(dkg_share_ids.clone()).await?;

        let mut public_key_packages = vec![];

        let dkg_shares_data = self.dkg_share_storage.get_batch_dkg_share_agg_data(dkg_share_ids.clone()).await?;

        for dkg_share_data in dkg_shares_data {
            match dkg_share_data {
                AggregatorDkgShareData { dkg_state: AggregatorDkgState::DkgFinalized { public_key_package }, } => {
                    public_key_packages.push(public_key_package);
                }
                _ => return Err(AggregatorError::InvalidUserState("User state is not DkgFinalized".to_string())),
            }
        }

        let dkg_responses = dkg_share_ids
            .into_iter()
            .zip(public_key_packages.into_iter())
            .map(|(dkg_share_id, public_key_package)| DkgResponse {
                dkg_share_id,
                public_key_package,
            })
            .collect::<Vec<DkgResponse>>();

        Ok(dkg_responses)
    }

    async fn sign_round_1(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
        message_hash: &[u8],
        metadata: SigningMetadata,
        tweak: Option<TweakBytes>,
    ) -> Result<(), AggregatorError> {
        debug!(dkg_share_id = ?dkg_share_id, session_id = %session_id, "Starting signing round 1");
        let dkg_share_data = self.dkg_share_storage.get_dkg_share_agg_data(dkg_share_id).await?;

        match dkg_share_data {
            Some(AggregatorDkgShareData {
                dkg_state:
                    AggregatorDkgState::DkgFinalized {
                        public_key_package: _public_key_package,
                    },
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

                debug!(dkg_share_id = ?dkg_share_id, session_id = %session_id, "Signing round 1 completed");
                Ok(())
            }
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }

    async fn sign_round_2(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
        tap_tweek: bool,
    ) -> Result<(), AggregatorError> {
        debug!(dkg_share_id = ?dkg_share_id, session_id = %session_id, "Starting signing round 2");
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
                let mut tweaked_public_key_package = match tweak.clone() {
                    Some(tweak) => public_key_package.clone().tweak(Some(tweak.to_vec())),
                    None => public_key_package.clone(),
                };
                if tap_tweek {
                    tweaked_public_key_package = tweaked_public_key_package.tweak::<Vec<u8>>(None);
                }
                let mut signature_shares = BTreeMap::new();
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let request = SignRound2Request {
                        dkg_share_id: *dkg_share_id,
                        session_id,
                        signing_package: signing_package.clone(),
                        tap_tweek,
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

                debug!(dkg_share_id = ?dkg_share_id, session_id = %session_id, "Signing round 2 completed");
                Ok(())
            }
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }

    pub async fn run_signing_flow(
        &self,
        dkg_share_id: Uuid,
        message_hash: &[u8],
        metadata: SigningMetadata,
        tweak: Option<TweakBytes>,
        tap_tweek: bool,
    ) -> Result<Signature, AggregatorError> {
        let session_id = global_utils::common_types::get_uuid();

        self.sign_round_1(&dkg_share_id, session_id, message_hash, metadata, tweak)
            .await?;
        self.sign_round_2(&dkg_share_id, session_id, tap_tweek).await?;

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
        dkg_share_id: Uuid,
        tweak: Option<TweakBytes>,
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
