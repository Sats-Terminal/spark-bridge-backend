use std::{collections::BTreeMap, sync::Arc};
use bitcoin::secp256k1::PublicKey;
use frost_secp256k1_tr::{Identifier, Signature, SigningPackage, keys, keys::Tweak};
use futures::future::join_all;
use uuid::Uuid;
use crate::{errors::AggregatorError, traits::*, types::*};

#[derive(Clone)]
pub struct FrostAggregator {
    verifiers: BTreeMap<Identifier, Arc<dyn SignerClient>>, // TODO: implement signer client
    user_key_storage: Arc<dyn AggregatorUserKeyStorage>,           // TODO: implement aggregator storage storage
    user_session_storage: Arc<dyn AggregatorUserSessionStorage>,           // TODO: implement aggregator storage storage
}

impl FrostAggregator {
    pub fn new(
        verifiers: BTreeMap<Identifier, Arc<dyn SignerClient>>,
        user_key_storage: Arc<dyn AggregatorUserKeyStorage>,
        user_session_storage: Arc<dyn AggregatorUserSessionStorage>,
    ) -> Self {
        Self {
            verifiers,
            user_key_storage,
            user_session_storage,
        }
    }

    async fn dkg_round_1(&self, user_public_key: PublicKey) -> Result<(), AggregatorError> {
        let key_info = self.user_key_storage.get_key_info(user_public_key.clone()).await?;

        match key_info {
            Some(_) => Err(AggregatorError::InvalidUserState("User state is not None".to_string())),
            None => {
                let signer_clients_request = DkgRound1Request {
                    user_public_key: user_public_key.clone(),
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

                self.user_key_storage
                    .set_key_info(
                        user_public_key.clone(),
                        AggregatorUserKeyInfo {
                            state: AggregatorUserKeyState::DkgRound1 {
                                round1_packages: verifier_responses,
                            },
                        },
                    )
                    .await?;

                Ok(())
            }
        }
    }

    async fn dkg_round_2(&self, user_public_key: PublicKey) -> Result<(), AggregatorError> {
        let key_info = self.user_key_storage
            .get_key_info(user_public_key.clone())
            .await?;

        match key_info {
            Some(AggregatorUserKeyInfo { state: AggregatorUserKeyState::DkgRound1 { round1_packages } }) => {
                let mut verifier_responses = BTreeMap::new();
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let mut packages = round1_packages.clone();
                    packages.remove(&verifier_id);
                    let signer_requests = DkgRound2Request {
                        user_public_key: user_public_key.clone(),
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

                self.user_key_storage
                    .set_key_info(
                        user_public_key.clone(),
                        AggregatorUserKeyInfo {
                            state: AggregatorUserKeyState::DkgRound2 {
                                round1_packages: round1_packages,
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

    async fn dkg_finalize(&self, user_public_key: PublicKey) -> Result<(), AggregatorError> {
        let key_info = self.user_key_storage.get_key_info(user_public_key.clone()).await?;

        match key_info {
            Some(AggregatorUserKeyInfo { state: AggregatorUserKeyState::DkgRound2 {
                round1_packages,
                round2_packages,
            }}) => {
                let mut public_key_packages = vec![];
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let mut verifier_round1_packages = round1_packages.clone();
                    verifier_round1_packages.remove(&verifier_id);
                    let request = DkgFinalizeRequest {
                        user_public_key: user_public_key.clone(),
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

                for (verifier_id, response) in join_handles {
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

                self.user_key_storage
                    .set_key_info(
                        user_public_key.clone(),
                        AggregatorUserKeyInfo {
                            state: AggregatorUserKeyState::DkgFinalized {
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

    pub async fn run_dkg_flow(&self, user_public_key: PublicKey) -> Result<keys::PublicKeyPackage, AggregatorError> {
        self.dkg_round_1(user_public_key.clone()).await?;
        self.dkg_round_2(user_public_key.clone()).await?;
        self.dkg_finalize(user_public_key.clone()).await?;

        let key_info = self.user_key_storage.get_key_info(user_public_key.clone()).await?;
        match key_info {
            Some(AggregatorUserKeyInfo { state: AggregatorUserKeyState::DkgFinalized { public_key_package } }) => Ok(public_key_package),
            _ => Err(AggregatorError::InvalidUserState(
                "User state is not DkgFinalized".to_string(),
            )),
        }
    }

    async fn sign_round_1(
        &self,
        user_public_key: PublicKey,
        session_id: Uuid,
        message_hash: &[u8],
        metadata: SigningMetadata,
        tweak: Option<&[u8]>,
    ) -> Result<(), AggregatorError> {
        let key_info = self.user_key_storage.get_key_info(user_public_key.clone()).await?;

        match key_info {
            Some(AggregatorUserKeyInfo { state: AggregatorUserKeyState::DkgFinalized { public_key_package } }) => {
                let mut commitments = BTreeMap::new();
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let request = SignRound1Request {
                        user_public_key: user_public_key.clone(),
                        metadata: metadata.clone(),
                        message_hash: message_hash.to_vec(),
                        session_id: session_id,
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

                self.user_session_storage
                    .set_session_info(
                        user_public_key.clone(),
                        session_id,
                        AggregatorUserSessionInfo {
                            tweak: tweak.map(|t| t.to_vec()),
                            message_hash: message_hash.to_vec(),
                            metadata: metadata,
                            state: AggregatorUserSessionState::SigningRound1 {
                                signing_package,
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

    async fn sign_round_2(&self, user_public_key: PublicKey, session_id: Uuid,) -> Result<(), AggregatorError> {
        let key_info = self.user_key_storage.get_key_info(user_public_key.clone()).await?;
        let mut session_info = self.user_session_storage.get_session_info(user_public_key.clone(), session_id.clone()).await?
            .ok_or(AggregatorError::InvalidUserState("Session state is not SigningRound1".to_string()))?;

        let public_key_package = match key_info {
            Some(AggregatorUserKeyInfo { state: AggregatorUserKeyState::DkgFinalized { public_key_package } }) => public_key_package,
            _ => return Err(AggregatorError::InvalidUserState("User state is not DkgFinalized".to_string())),
        };

        match session_info.clone() {
            AggregatorUserSessionInfo { 
                tweak,
                message_hash,
                metadata: _,
                state: AggregatorUserSessionState::SigningRound1 {
                signing_package,
            }} => {
                let tweaked_public_key_package = match tweak.clone() {
                    Some(tweak) => public_key_package.clone().tweak(Some(tweak.to_vec())),
                    None => public_key_package.clone(),
                };
                let mut signature_shares = BTreeMap::new();
                let mut join_handles = vec![];

                for (verifier_id, signer_client) in self.verifiers.clone() {
                    let request = SignRound2Request {
                        user_public_key: user_public_key.clone(),
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

                session_info.state = AggregatorUserSessionState::SigningRound2 {
                    signature,
                };
                self.user_session_storage
                    .set_session_info(
                        user_public_key.clone(),
                        session_id,
                        session_info,
                    )
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
        user_id: PublicKey,
        message_hash: &[u8],
        metadata: SigningMetadata,
        tweak: Option<&[u8]>,
    ) -> Result<Signature, AggregatorError> {
        let session_id = global_utils::common_types::get_uuid();

        self.sign_round_1(user_id.clone(), session_id.clone(), message_hash, metadata, tweak).await?;
        self.sign_round_2(user_id.clone(), session_id.clone()).await?;

        let session_info = self.user_session_storage.get_session_info(user_id.clone(), session_id.clone()).await?;
        let state = session_info.ok_or(AggregatorError::InvalidUserState("Session state is not SigningRound2".to_string()))?.state;
        
        match state {
            AggregatorUserSessionState::SigningRound2 { signature } => Ok(signature),
            _ => Err(AggregatorError::InvalidUserState("Session state is not SigningRound2".to_string())),
        }
    }
}
