use crate::{errors::SignerError, traits::*, types::*};
use frost_secp256k1_tr::{Identifier, keys::Tweak};
use rand_core::OsRng;
use std::{collections::BTreeSet, sync::Arc};
use tokio::sync::Mutex;
use tracing::debug;
use uuid::Uuid;

#[derive(Clone)]
pub struct FrostSigner {
    dkg_share_storage: Arc<dyn SignerDkgShareStorage>,
    identifier: Identifier,
    sign_session_storage: Arc<dyn SignerSignSessionStorage>,
    total_participants: u16,
    threshold: u16,
    locked_dkg_share_ids: Arc<Mutex<BTreeSet<Uuid>>>,
}

impl FrostSigner {
    pub fn new(
        identifier: u16,
        dkg_share_storage: Arc<dyn SignerDkgShareStorage>,
        sign_session_storage: Arc<dyn SignerSignSessionStorage>,
        total_participants: u16,
        threshold: u16,
    ) -> Result<Self, SignerError> {
        Ok(Self {
            dkg_share_storage,
            sign_session_storage,
            identifier: identifier
                .try_into()
                .map_err(|e| SignerError::Internal(format!("Failed to convert identifier: {}", e)))?,
            total_participants,
            threshold,
            locked_dkg_share_ids: Arc::new(Mutex::new(BTreeSet::new())),
        })
    }

    pub async fn lock_dkg_share_id(&self, dkg_share_id: &Uuid) -> Result<(), SignerError> {
        let mut locked_dkg_share_ids = self.locked_dkg_share_ids.lock().await;
        if locked_dkg_share_ids.contains(dkg_share_id) {
            return Err(SignerError::DkgShareIdAlreadyExists(*dkg_share_id));
        }
        locked_dkg_share_ids.insert(dkg_share_id.clone());
        Ok(())
    }

    pub async fn unlock_dkg_share_id(&self, dkg_share_id: &Uuid) -> Result<(), SignerError> {
        let mut locked_dkg_share_ids = self.locked_dkg_share_ids.lock().await;
        let removed = locked_dkg_share_ids.remove(dkg_share_id);
        if !removed {
            return Err(SignerError::DkgShareIdNotFound(*dkg_share_id));
        }
        Ok(())
    }

    pub async fn dkg_round_1(&self, request: DkgRound1Request) -> Result<DkgRound1Response, SignerError> {
        debug!(dkg_share_id = ?request.dkg_share_id, identifier = ?self.identifier, "Started DKG round 1");
        self.lock_dkg_share_id(&request.dkg_share_id).await?;

        let dkg_share_id = request.dkg_share_id;
        let dkg_share_data = self.dkg_share_storage.get_dkg_share_signer_data(&dkg_share_id).await?;

        match dkg_share_data {
            None => {
                let (secret_package, package) = frost_secp256k1_tr::keys::dkg::part1(
                    self.identifier,
                    self.total_participants,
                    self.threshold,
                    OsRng,
                )
                .map_err(|e| SignerError::Internal(format!("DKG round1 failed: {e}")))?;

                self.dkg_share_storage
                    .set_dkg_share_signer_data(
                        &dkg_share_id,
                        SignerDkgShareIdData {
                            dkg_state: SignerDkgState::DkgRound1 {
                                round1_secret_package: secret_package,
                            },
                        },
                    )
                    .await?;

                debug!(dkg_share_id = ?dkg_share_id, identifier = ?self.identifier, "DKG round 1 completed");
                Ok(DkgRound1Response {
                    round1_package: package,
                })
            }
            _ => {
                self.unlock_dkg_share_id(&dkg_share_id).await?;
                Err(SignerError::DkgShareIdAlreadyExists(dkg_share_id))
            }
        }
    }

    pub async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response, SignerError> {
        let dkg_share_id = request.dkg_share_id;
        debug!(dkg_share_id = ?dkg_share_id, identifier = ?self.identifier, "Started DKG 2 round");
        let dkg_share_data = self.dkg_share_storage.get_dkg_share_signer_data(&dkg_share_id).await?;

        match dkg_share_data {
            Some(SignerDkgShareIdData {
                dkg_state: SignerDkgState::DkgRound1 { round1_secret_package },
            }) => {
                let (secret_package, packages) =
                    frost_secp256k1_tr::keys::dkg::part2(round1_secret_package.clone(), &request.round1_packages)
                        .map_err(|e| SignerError::Internal(format!("DKG round2 failed: {e}")))?;

                self.dkg_share_storage
                    .set_dkg_share_signer_data(
                        &dkg_share_id,
                        SignerDkgShareIdData {
                            dkg_state: SignerDkgState::DkgRound2 {
                                round2_secret_package: secret_package,
                                round1_packages: request.round1_packages,
                            },
                        },
                    )
                    .await?;

                debug!(dkg_share_id = ?dkg_share_id, identifier = ?self.identifier, "DKG 2 round completed");
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
        let dkg_share_id = request.dkg_share_id;
        debug!(dkg_share_id = ?dkg_share_id, identifier = ?self.identifier, "Started DKG finalize round");
        let dkg_share_data = self.dkg_share_storage.get_dkg_share_signer_data(&dkg_share_id).await?;

        match dkg_share_data {
            Some(SignerDkgShareIdData {
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

                self.dkg_share_storage
                    .set_dkg_share_signer_data(
                        &dkg_share_id,
                        SignerDkgShareIdData {
                            dkg_state: SignerDkgState::DkgFinalized {
                                key_package: Box::new(key_package),
                            },
                        },
                    )
                    .await?;

                self.unlock_dkg_share_id(&dkg_share_id).await?;

                debug!(dkg_share_id = ?dkg_share_id, identifier = ?self.identifier, "DKG finalize completed");
                Ok(DkgFinalizeResponse { public_key_package })
            }
            _ => Err(SignerError::InvalidUserState(
                "User key state is not DkgRound2".to_string(),
            )),
        }
    }

    pub async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response, SignerError> {
        let dkg_share_id = request.dkg_share_id;
        let session_id = request.session_id;
        let tweak = request.tweak;
        let message_hash = request.message_hash;
        let metadata = request.metadata;

        debug!(dkg_share_id = ?dkg_share_id, session_id = %session_id, "Starting signing round 1");

        let dkg_share_data = self.dkg_share_storage.get_dkg_share_signer_data(&dkg_share_id).await?;

        match dkg_share_data {
            Some(SignerDkgShareIdData {
                dkg_state: SignerDkgState::DkgFinalized { key_package },
            }) => {
                let tweak_key_package = match tweak.clone() {
                    Some(tweak) => Box::new(key_package.clone().tweak(Some(tweak.to_vec()))),
                    None => key_package.clone(),
                };
                let (nonces, commitments) =
                    frost_secp256k1_tr::round1::commit(tweak_key_package.signing_share(), &mut OsRng);

                self.sign_session_storage
                    .set_sign_data(
                        &dkg_share_id,
                        session_id,
                        SignerSignData {
                            tweak,
                            message_hash,
                            metadata,
                            sign_state: SignerSignState::SigningRound1 {
                                nonces: Box::new(nonces),
                            },
                        },
                    )
                    .await?;

                debug!(dkg_share_id = ?dkg_share_id, session_id = %session_id, "Signing round 1 completed");
                Ok(SignRound1Response { commitments })
            }
            _ => Err(SignerError::InvalidUserState(
                "User key state is not DkgFinalized".to_string(),
            )),
        }
    }

    pub async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response, SignerError> {
        let dkg_share_id = request.dkg_share_id;
        let session_id = request.session_id;

        debug!(dkg_share_id = ?dkg_share_id, session_id = %session_id, "Starting signing round 2");

        let dkg_share_data = self.dkg_share_storage.get_dkg_share_signer_data(&dkg_share_id).await?;

        let key_package = match dkg_share_data {
            Some(SignerDkgShareIdData {
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
            .get_sign_data(&dkg_share_id, session_id)
            .await?
            .ok_or(SignerError::InvalidUserState(
                "Session state is not SigningRound1".to_string(),
            ))?;

        assert_eq!(sign_data.message_hash, request.signing_package.message().clone());

        match sign_data.sign_state {
            SignerSignState::SigningRound1 { nonces } => {
                let mut tweak_key_package = match sign_data.tweak.clone() {
                    Some(tweak) => Box::new(key_package.clone().tweak(Some(tweak.to_vec()))),
                    None => key_package.clone(),
                };
                if request.tap_tweek {
                    tweak_key_package = Box::new(tweak_key_package.tweak::<Vec<u8>>(None));
                }
                let signature_share =
                    frost_secp256k1_tr::round2::sign(&request.signing_package, &nonces, &tweak_key_package)
                        .map_err(|e| SignerError::Internal(format!("Sign round2 failed: {e}")))?;

                sign_data.sign_state = SignerSignState::SigningRound2 {
                    signature_share: Box::new(signature_share),
                };

                self.sign_session_storage
                    .set_sign_data(&dkg_share_id, session_id, sign_data)
                    .await?;

                debug!(dkg_share_id= ?dkg_share_id, session_id = %session_id, "Signing round 2 finished");

                Ok(SignRound2Response { signature_share })
            }
            _ => Err(SignerError::InvalidUserState(
                "User session state is not SigningRound1".to_string(),
            )),
        }
    }
}
