use std::sync::Arc;

use frost_secp256k1_tr::{Identifier, keys::Tweak};

use rand_core::OsRng;

use crate::{config::SignerConfig, errors::SignerError, traits::*};

#[derive(Clone)]
pub struct FrostSigner {
    config: SignerConfig,
    user_storage: Arc<dyn SignerUserStorage>, // TODO: implement signer storage
    identifier: Identifier,
}

impl FrostSigner {
    pub fn new(config: SignerConfig, user_storage: Arc<dyn SignerUserStorage>) -> Self {
        Self {
            config: config.clone(),
            user_storage,
            identifier: config.identifier.try_into().unwrap(),
        }
    }

    pub async fn dkg_round_1(&self, request: DkgRound1Request) -> Result<DkgRound1Response, SignerError> {
        let user_id = request.user_id;
        let state = self.user_storage.get_user_state(user_id.clone()).await?;

        match state {
            None => {
                let (secret_package, package) = frost_secp256k1_tr::keys::dkg::part1(
                    self.identifier,
                    self.config.total_participants,
                    self.config.threshold,
                    &mut OsRng,
                )
                .map_err(|e| SignerError::Internal(format!("DKG round1 failed: {e}")))?;

                self.user_storage
                    .set_user_state(
                        user_id.clone(),
                        SignerUserState::DkgRound1 {
                            round1_secret_package: secret_package,
                        },
                    )
                    .await?;

                Ok(DkgRound1Response {
                    round1_package: package,
                })
            }
            _ => {
                return Err(SignerError::InvalidUserState(
                    "User state is not SigningRound1".to_string(),
                ));
            }
        }
    }

    pub async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response, SignerError> {
        let user_id = request.user_id;
        let state = self.user_storage.get_user_state(user_id.clone()).await?;

        match state {
            Some(SignerUserState::DkgRound1 { round1_secret_package }) => {
                let (secret_package, packages) =
                    frost_secp256k1_tr::keys::dkg::part2(round1_secret_package.clone(), &request.round1_packages)
                        .map_err(|e| SignerError::Internal(format!("DKG round2 failed: {e}")))?;

                self.user_storage
                    .set_user_state(
                        user_id.clone(),
                        SignerUserState::DkgRound2 {
                            round2_secret_package: secret_package,
                            round1_packages: request.round1_packages,
                        },
                    )
                    .await?;

                Ok(DkgRound2Response {
                    round2_packages: packages,
                })
            }
            _ => {
                return Err(SignerError::InvalidUserState(
                    "User state is not SigningRound1".to_string(),
                ));
            }
        }
    }

    pub async fn dkg_finalize(&self, request: DkgFinalizeRequest) -> Result<DkgFinalizeResponse, SignerError> {
        let user_id = request.user_id;
        let state = self.user_storage.get_user_state(user_id.clone()).await?;

        match state {
            Some(SignerUserState::DkgRound2 {
                round2_secret_package,
                round1_packages,
            }) => {
                let (key_package, public_key_package) = frost_secp256k1_tr::keys::dkg::part3(
                    &round2_secret_package,
                    &round1_packages,
                    &request.round2_packages,
                )
                .map_err(|e| SignerError::Internal(format!("DKG finalize failed: {e}")))?;

                self.user_storage
                    .set_user_state(user_id.clone(), SignerUserState::DkgFinalized { key_package })
                    .await?;
                Ok(DkgFinalizeResponse { public_key_package })
            }
            _ => {
                return Err(SignerError::InvalidUserState(
                    "User state is not SigningRound1".to_string(),
                ));
            }
        }
    }

    pub async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response, SignerError> {
        let user_id = request.user_id;
        let tweak = request.tweak;
        let state = self.user_storage.get_user_state(user_id.clone()).await?;

        match state {
            Some(SignerUserState::DkgFinalized { key_package }) => {
                let tweak_key_package = key_package.clone().tweak(Some(tweak.clone()));
                let (nonces, commitments) =
                    frost_secp256k1_tr::round1::commit(tweak_key_package.signing_share(), &mut OsRng);

                self.user_storage
                    .set_user_state(
                        user_id.clone(),
                        SignerUserState::SigningRound1 {
                            key_package: key_package,
                            tweak: tweak,
                            nonces,
                        },
                    )
                    .await?;
                Ok(SignRound1Response { user_id, commitments })
            }
            _ => {
                return Err(SignerError::InvalidUserState(
                    "User state is not SigningRound1".to_string(),
                ));
            }
        }
    }

    pub async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response, SignerError> {
        let user_id = request.user_id;
        let state = self.user_storage.get_user_state(user_id.clone()).await?;

        match state {
            Some(SignerUserState::SigningRound1 {
                key_package,
                tweak,
                nonces,
            }) => {
                let tweak_key_package = key_package.clone().tweak(Some(tweak.clone()));
                let signature_share =
                    frost_secp256k1_tr::round2::sign(&request.signing_package, &nonces, &tweak_key_package)
                        .map_err(|e| SignerError::Internal(format!("Sign round2 failed: {e}")))?;

                self.user_storage
                    .set_user_state(
                        user_id.clone(),
                        SignerUserState::SigningRound2 {
                            tweak: tweak.clone(),
                            key_package: key_package.clone(),
                            signature_share,
                        },
                    )
                    .await?;

                self.user_storage
                    .set_user_state(
                        user_id.clone(),
                        SignerUserState::DkgFinalized {
                            key_package: key_package.clone(),
                        },
                    )
                    .await?;
                Ok(SignRound2Response { signature_share })
            }
            _ => {
                return Err(SignerError::InvalidUserState(
                    "User state is not SigningRound1".to_string(),
                ));
            }
        }
    }
}
