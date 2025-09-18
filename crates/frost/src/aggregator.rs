use crate::{errors::AggregatorError, traits::*, types::*};
use frost_secp256k1_tr::{Identifier, Signature, SigningPackage, keys, keys::Tweak};
use futures::future::join_all;
use std::{collections::BTreeMap, sync::Arc};
use tracing::instrument;
use uuid::Uuid;
use crate::types::Nonce;

#[derive(Clone, Debug)]
pub struct FrostAggregator {
    verifiers: BTreeMap<Identifier, Arc<dyn SignerClient>>, // TODO: implement signer client
    musig_id_storage: Arc<dyn AggregatorMusigIdStorage>,    // TODO: implement aggregator storage storage
    sign_session_storage: Arc<dyn AggregatorSignSessionStorage>, // TODO: implement aggregator storage storage
}

impl FrostAggregator {
    pub fn new(
        verifiers: BTreeMap<Identifier, Arc<dyn SignerClient>>,
        musig_id_storage: Arc<dyn AggregatorMusigIdStorage>,
        sign_session_storage: Arc<dyn AggregatorSignSessionStorage>,
    ) -> Self {
        Self {
            verifiers,
            musig_id_storage,
            sign_session_storage,
        }
    }

    async fn dkg_round_1(&self, musig_id: &MusigId) -> Result<(), AggregatorError> {
        let musig_id_data = self.musig_id_storage.get_musig_id_data(musig_id).await?;

        match musig_id_data {
            Some(_) => Err(AggregatorError::InvalidUserState("User state is not None".to_string())),
            None => {
                let signer_clients_request = DkgRound1Request {
                    musig_id: musig_id.clone(),
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

                self.musig_id_storage
                    .set_musig_id_data(
                        musig_id,
                        AggregatorMusigIdData {
                            dkg_state: AggregatorDkgState::DkgRound1 {
                                round1_packages: verifier_responses,
                            },
                        },
                    )
                    .await?;

                Ok(())
            }
        }
    }

    async fn dkg_round_2(&self, musig_id: &MusigId) -> Result<(), AggregatorError> {
        let musig_id_data = self.musig_id_storage.get_musig_id_data(&musig_id).await?;

        match musig_id_data {
            Some(AggregatorMusigIdData {
                dkg_state: AggregatorDkgState::DkgRound1 { round1_packages },
            }) => {
                let mut verifier_responses = BTreeMap::new();
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let mut packages = round1_packages.clone();
                    packages.remove(&verifier_id);
                    let signer_requests = DkgRound2Request {
                        musig_id: musig_id.clone(),
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

                self.musig_id_storage
                    .set_musig_id_data(
                        musig_id,
                        AggregatorMusigIdData {
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
                "User key state is not DkgRound2".to_string(),
            )),
        }
    }

    async fn dkg_finalize(&self, musig_id: &MusigId) -> Result<(), AggregatorError> {
        let musig_id_data = self.musig_id_storage.get_musig_id_data(musig_id).await?;

        match musig_id_data {
            Some(AggregatorMusigIdData {
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
                        musig_id: musig_id.clone(),
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

                self.musig_id_storage
                    .set_musig_id_data(
                        musig_id,
                        AggregatorMusigIdData {
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

    #[instrument(skip(self), level = "trace", ret)]
    pub async fn run_dkg_flow(&self, musig_id: &MusigId) -> Result<keys::PublicKeyPackage, AggregatorError> {
        self.dkg_round_1(musig_id).await?;
        self.dkg_round_2(musig_id).await?;
        self.dkg_finalize(musig_id).await?;

        let musig_id_data = self.musig_id_storage.get_musig_id_data(musig_id).await?;
        match musig_id_data {
            Some(AggregatorMusigIdData {
                dkg_state: AggregatorDkgState::DkgFinalized { public_key_package },
            }) => Ok(public_key_package),
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }

    async fn sign_round_1(
        &self,
        musig_id: &MusigId,
        session_id: Uuid,
        message_hash: &[u8],
        metadata: SigningMetadata,
        tweak: Option<Nonce>,
    ) -> Result<(), AggregatorError> {
        let musig_id_data = self.musig_id_storage.get_musig_id_data(musig_id).await?;

        match musig_id_data {
            Some(AggregatorMusigIdData {
                dkg_state: AggregatorDkgState::DkgFinalized { public_key_package },
            }) => {
                let mut commitments = BTreeMap::new();
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let request = SignRound1Request {
                        musig_id: musig_id.clone(),
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

                let signing_package = SigningPackage::new(commitments.clone(), message_hash.clone());

                self.sign_session_storage
                    .set_sign_data(
                        musig_id,
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

    async fn sign_round_2(&self, musig_id: &MusigId, session_id: Uuid) -> Result<(), AggregatorError> {
        let musig_id_data = self.musig_id_storage.get_musig_id_data(musig_id).await?;
        let mut sign_data = self
            .sign_session_storage
            .get_sign_data(musig_id, session_id)
            .await?
            .ok_or(AggregatorError::InvalidUserState(
                "Session state is not SigningRound1".to_string(),
            ))?;

        let public_key_package = match musig_id_data {
            Some(AggregatorMusigIdData {
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
                        musig_id: musig_id.clone(),
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
                    .set_sign_data(musig_id, session_id, sign_data)
                    .await?;

                Ok(())
            }
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }

    pub async fn run_signing_flow(
        &self,
        musig_id: MusigId,
        message_hash: &[u8],
        metadata: SigningMetadata,
        tweak: Option<Nonce>,
    ) -> Result<Signature, AggregatorError> {
        let session_id = global_utils::common_types::get_uuid();

        self.sign_round_1(&musig_id, session_id, message_hash, metadata, tweak)
            .await?;
        self.sign_round_2(&musig_id, session_id).await?;

        let sign_data = self.sign_session_storage.get_sign_data(&musig_id, session_id).await?;
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
        musig_id: MusigId,
        tweak: Option<Nonce>,
    ) -> Result<keys::PublicKeyPackage, AggregatorError> {
        let musig_id_data = self.musig_id_storage.get_musig_id_data(&musig_id).await?;

        match musig_id_data {
            Some(AggregatorMusigIdData {
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
