use crate::{errors::SignerError, traits::*, types::*};
use frost_secp256k1_tr::{Identifier, keys::Tweak};
use rand_core::OsRng;
use std::{collections::BTreeSet, sync::Arc};
use tokio::sync::Mutex;
use tracing::debug;

#[derive(Clone, Debug)]
pub struct FrostSigner {
    musig_id_storage: Arc<dyn SignerMusigIdStorage>, // TODO: implement signer storage
    identifier: Identifier,
    sign_session_storage: Arc<dyn SignerSignSessionStorage>,
    total_participants: u16,
    threshold: u16,
    locked_musig_ids: Arc<Mutex<BTreeSet<MusigId>>>,
}

impl FrostSigner {
    pub fn new(
        identifier: u16,
        musig_id_storage: Arc<dyn SignerMusigIdStorage>,
        sign_session_storage: Arc<dyn SignerSignSessionStorage>,
        total_participants: u16,
        threshold: u16,
    ) -> Self {
        Self {
            musig_id_storage,
            sign_session_storage,
            identifier: identifier.try_into().unwrap(),
            total_participants,
            threshold,
            locked_musig_ids: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub async fn lock_musig_id(&self, musig_id: &MusigId) -> Result<(), SignerError> {
        let mut locked_musig_ids = self.locked_musig_ids.lock().await;
        if locked_musig_ids.contains(musig_id) {
            return Err(SignerError::MusigAlreadyExists(format!(
                "Musig id already exists: {:?}",
                musig_id
            )));
        }
        locked_musig_ids.insert(musig_id.clone());
        Ok(())
    }

    pub async fn unlock_musig_id(&self, musig_id: &MusigId) -> Result<(), SignerError> {
        let mut locked_musig_ids = self.locked_musig_ids.lock().await;
        let removed = locked_musig_ids.remove(musig_id);
        if !removed {
            return Err(SignerError::MusigNotFound(format!(
                "Something bad went wrong: {:?}",
                musig_id
            )));
        }
        Ok(())
    }

    pub async fn dkg_round_1(&self, request: DkgRound1Request) -> Result<DkgRound1Response, SignerError> {
        debug!(musig_id = ?request.musig_id, identifier = ?self.identifier, "Started DKG round 1");
        self.lock_musig_id(&request.musig_id).await?;

        let musig_id = request.musig_id;
        let musig_id_data = self.musig_id_storage.get_musig_id_data(&musig_id).await?;

        match musig_id_data {
            None => {
                let (secret_package, package) = frost_secp256k1_tr::keys::dkg::part1(
                    self.identifier,
                    self.total_participants,
                    self.threshold,
                    OsRng,
                )
                .map_err(|e| SignerError::Internal(format!("DKG round1 failed: {e}")))?;

                self.musig_id_storage
                    .set_musig_id_data(
                        &musig_id,
                        SignerMusigIdData {
                            dkg_state: SignerDkgState::DkgRound1 {
                                round1_secret_package: secret_package,
                            },
                        },
                    )
                    .await?;

                debug!(musig_id = ?musig_id, identifier = ?self.identifier, "DKG round 1 completed");
                Ok(DkgRound1Response {
                    round1_package: package,
                })
            }
            _ => {
                self.unlock_musig_id(&musig_id).await?;
                Err(SignerError::MusigAlreadyExists(format!(
                    "Musig id already exists: {:?}",
                    musig_id
                )))
            }
        }
    }

    pub async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response, SignerError> {
        let musig_id = request.musig_id;
        debug!(musig_id = ?musig_id, identifier = ?self.identifier, "Started DKG 2 round");
        let musig_id_data = self.musig_id_storage.get_musig_id_data(&musig_id).await?;

        match musig_id_data {
            Some(SignerMusigIdData {
                dkg_state: SignerDkgState::DkgRound1 { round1_secret_package },
            }) => {
                let (secret_package, packages) =
                    frost_secp256k1_tr::keys::dkg::part2(round1_secret_package.clone(), &request.round1_packages)
                        .map_err(|e| SignerError::Internal(format!("DKG round2 failed: {e}")))?;

                self.musig_id_storage
                    .set_musig_id_data(
                        &musig_id,
                        SignerMusigIdData {
                            dkg_state: SignerDkgState::DkgRound2 {
                                round2_secret_package: secret_package,
                                round1_packages: request.round1_packages,
                            },
                        },
                    )
                    .await?;

                debug!(musig_id = ?musig_id, identifier = ?self.identifier, "DKG 2 round completed");
                Ok(DkgRound2Response {
                    round2_packages: packages,
                })
            }
            _ => Err(SignerError::InvalidUserState(
                "User key state is not DkgRound1".to_string(),
            )),
        }
    }

    pub async fn dkg_finalize(&self, request: DkgFinalizeRequest) -> Result<DkgFinalizeResponse, SignerError> {
        let musig_id = request.musig_id;
        debug!(musig_id = ?musig_id, identifier = ?self.identifier, "Started DKG finalize round");
        let musig_id_data = self.musig_id_storage.get_musig_id_data(&musig_id).await?;

        match musig_id_data {
            Some(SignerMusigIdData {
                dkg_state:
                    SignerDkgState::DkgRound2 {
                        round2_secret_package,
                        round1_packages,
                    },
            }) => {
                let (key_package, public_key_package) = frost_secp256k1_tr::keys::dkg::part3(
                    &round2_secret_package,
                    &round1_packages,
                    &request.round2_packages,
                )
                .map_err(|e| SignerError::Internal(format!("DKG finalize failed: {e}")))?;

                self.musig_id_storage
                    .set_musig_id_data(
                        &musig_id,
                        SignerMusigIdData {
                            dkg_state: SignerDkgState::DkgFinalized { key_package },
                        },
                    )
                    .await?;

                self.unlock_musig_id(&musig_id).await?;

                debug!(musig_id = ?musig_id, identifier = ?self.identifier, "DKG finalize completed");
                Ok(DkgFinalizeResponse { public_key_package })
            }
            _ => Err(SignerError::InvalidUserState(
                "User key state is not DkgRound2".to_string(),
            )),
        }
    }

    pub async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response, SignerError> {
        let musig_id = request.musig_id;
        let session_id = request.session_id;
        let tweak = request.tweak;
        let message_hash = request.message_hash;
        let metadata = request.metadata;

        debug!(musig_id = ?musig_id, session_id = %session_id, "Starting signing round 1");

        let musig_id_data = self.musig_id_storage.get_musig_id_data(&musig_id).await?;

        match musig_id_data {
            Some(SignerMusigIdData {
                dkg_state: SignerDkgState::DkgFinalized { key_package },
            }) => {
                let tweak_key_package = match tweak.clone() {
                    Some(tweak) => key_package.clone().tweak(Some(tweak.to_vec())),
                    None => key_package.clone(),
                };
                let (nonces, commitments) =
                    frost_secp256k1_tr::round1::commit(tweak_key_package.signing_share(), &mut OsRng);

                self.sign_session_storage
                    .set_sign_data(
                        &musig_id,
                        session_id,
                        SignerSignData {
                            tweak,
                            message_hash,
                            metadata,
                            sign_state: SignerSignState::SigningRound1 { nonces },
                        },
                    )
                    .await?;

                debug!(musig_id = ?musig_id, session_id = %session_id, "Signing round 1 completed");
                Ok(SignRound1Response { commitments })
            }
            _ => Err(SignerError::InvalidUserState(
                "User key state is not DkgFinalized".to_string(),
            )),
        }
    }

    pub async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response, SignerError> {
        let musig_id = request.musig_id;
        let session_id = request.session_id;

        debug!(musig_id = ?musig_id, session_id = %session_id, "Starting signing round 2");

        let musig_id_data = self.musig_id_storage.get_musig_id_data(&musig_id).await?;

        let key_package = match musig_id_data {
            Some(SignerMusigIdData {
                dkg_state: SignerDkgState::DkgFinalized { key_package },
            }) => key_package,
            _ => {
                return Err(SignerError::InvalidUserState(
                    "User key state is not DkgFinalized".to_string(),
                ));
            }
        };

        let mut sign_data = self
            .sign_session_storage
            .get_sign_data(&musig_id, session_id)
            .await?
            .ok_or(SignerError::InvalidUserState(
                "Session state is not SigningRound1".to_string(),
            ))?;

        assert_eq!(sign_data.message_hash, request.signing_package.message().clone());

        match sign_data.sign_state {
            SignerSignState::SigningRound1 { nonces } => {
                let tweak_key_package = match sign_data.tweak.clone() {
                    Some(tweak) => key_package.clone().tweak(Some(tweak.to_vec())),
                    None => key_package.clone(),
                };
                let signature_share =
                    frost_secp256k1_tr::round2::sign(&request.signing_package, &nonces, &tweak_key_package)
                        .map_err(|e| SignerError::Internal(format!("Sign round2 failed: {e}")))?;

                sign_data.sign_state = SignerSignState::SigningRound2 { signature_share };

                self.sign_session_storage
                    .set_sign_data(&musig_id, session_id, sign_data)
                    .await?;

                debug!(musig_id = ?musig_id, session_id = %session_id, "Signing round 2 finished");

                Ok(SignRound2Response { signature_share })
            }
            _ => Err(SignerError::InvalidUserState(
                "User session state is not SigningRound1".to_string(),
            )),
        }
    }

    pub fn healthcheck(&self) -> Result<(), SignerError> {
        // TODO: maybe perform some internal checks
        Ok(())
    }
}
