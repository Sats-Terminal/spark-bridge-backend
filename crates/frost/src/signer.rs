use std::sync::Arc;
use frost_secp256k1_tr::{Identifier, keys::Tweak};
use rand_core::OsRng;
use crate::{errors::SignerError, traits::*, types::*};

#[derive(Clone)]
pub struct FrostSigner {
    user_key_storage: Arc<dyn SignerUserKeyStorage>, // TODO: implement signer storage
    identifier: Identifier,
    user_session_storage: Arc<dyn SignerUserSessionStorage>,
    total_participants: u16,
    threshold: u16,
}

impl FrostSigner {
    pub fn new(
        identifier: u16, 
        user_key_storage: Arc<dyn SignerUserKeyStorage>, 
        user_session_storage: Arc<dyn SignerUserSessionStorage>,
        total_participants: u16,
        threshold: u16,
    ) -> Self {
        Self {
            user_key_storage,
            user_session_storage,
            identifier: identifier.try_into().unwrap(),
            total_participants,
            threshold,
        }
    }

    pub async fn dkg_round_1(&self, request: DkgRound1Request) -> Result<DkgRound1Response, SignerError> {
        let user_public_key = request.user_public_key;
        let key_info = self.user_key_storage.get_key_info(user_public_key.clone()).await?;

        match key_info {
            None => {
                let (secret_package, package) = frost_secp256k1_tr::keys::dkg::part1(
                    self.identifier,
                    self.total_participants,
                    self.threshold,
                    &mut OsRng,
                )
                .map_err(|e| SignerError::Internal(format!("DKG round1 failed: {e}")))?;

                self.user_key_storage
                    .set_key_info(
                        user_public_key.clone(),
                        SignerUserKeyInfo {
                            state: SignerUserKeyState::DkgRound1 {
                                round1_secret_package: secret_package,
                            },
                        },
                    )
                    .await?;

                Ok(DkgRound1Response {
                    round1_package: package,
                })
            }
            _ => {
                Err(SignerError::InvalidUserState(
                    "User key state is not Null".to_string(),
                ))
            }
        }
    }

    pub async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response, SignerError> {
        let user_public_key = request.user_public_key;
        let key_info = self.user_key_storage.get_key_info(user_public_key.clone()).await?;

        match key_info {
            Some(SignerUserKeyInfo { state: SignerUserKeyState::DkgRound1 { round1_secret_package } }) => {
                let (secret_package, packages) =
                    frost_secp256k1_tr::keys::dkg::part2(round1_secret_package.clone(), &request.round1_packages)
                        .map_err(|e| SignerError::Internal(format!("DKG round2 failed: {e}")))?;

                self.user_key_storage
                    .set_key_info(
                        user_public_key.clone(),
                        SignerUserKeyInfo {
                            state: SignerUserKeyState::DkgRound2 {
                                round2_secret_package: secret_package,
                                round1_packages: request.round1_packages,
                            },
                        },
                    )
                    .await?;

                Ok(DkgRound2Response {
                    round2_packages: packages,
                })
            }
            _ => {
                Err(SignerError::InvalidUserState(
                    "User key state is not DkgRound1".to_string(),
                ))
            }
        }
    }

    pub async fn dkg_finalize(&self, request: DkgFinalizeRequest) -> Result<DkgFinalizeResponse, SignerError> {
        let user_public_key = request.user_public_key;
        let key_info = self.user_key_storage.get_key_info(user_public_key.clone()).await?;

        match key_info {
            Some(SignerUserKeyInfo { state: SignerUserKeyState::DkgRound2 {
                round2_secret_package,
                round1_packages,
            }}) => {
                let (key_package, public_key_package) = frost_secp256k1_tr::keys::dkg::part3(
                    &round2_secret_package,
                    &round1_packages,
                    &request.round2_packages,
                )
                .map_err(|e| SignerError::Internal(format!("DKG finalize failed: {e}")))?;

                self.user_key_storage
                    .set_key_info(user_public_key.clone(), SignerUserKeyInfo { state: SignerUserKeyState::DkgFinalized { key_package } })
                    .await?;
                Ok(DkgFinalizeResponse { public_key_package })
            }
            _ => {
                Err(SignerError::InvalidUserState(
                    "User key state is not DkgRound2".to_string(),
                ))
            }
        }
    }

    pub async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response, SignerError> {
        let user_public_key = request.user_public_key;
        let session_id = request.session_id.clone();
        let tweak = request.tweak;
        let message_hash = request.message_hash;
        let metadata = request.metadata;

        let key_info = self.user_key_storage.get_key_info(user_public_key.clone()).await?;

        match key_info {
            Some(SignerUserKeyInfo { state: SignerUserKeyState::DkgFinalized { key_package } }) => {
                let tweak_key_package = match tweak.clone() {
                    Some(tweak) => key_package.clone().tweak(Some(tweak.to_vec())),
                    None => key_package.clone(),
                };
                let (nonces, commitments) =
                    frost_secp256k1_tr::round1::commit(tweak_key_package.signing_share(), &mut OsRng);

                self.user_session_storage
                    .set_session_info(
                        user_public_key.clone(),
                        session_id.clone(),
                        SignerUserSessionInfo {
                            tweak,
                            message_hash,
                            metadata,
                            state: SignerUserSessionState::SigningRound1 {
                                nonces,
                            },
                        },
                    )
                    .await?;
                Ok(SignRound1Response { user_public_key, session_id, commitments })
            }
            _ => {
                Err(SignerError::InvalidUserState(
                    "User key state is not DkgFinalized".to_string(),
                ))
            }
        }
    }

    pub async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response, SignerError> {
        let user_public_key = request.user_public_key;
        let session_id = request.session_id.clone();
        
        let key_info = self.user_key_storage.get_key_info(user_public_key.clone()).await?;

        let key_package = match key_info {
            Some(SignerUserKeyInfo { state: SignerUserKeyState::DkgFinalized { key_package } }) => key_package,
            _ => return Err(SignerError::InvalidUserState("User key state is not DkgFinalized".to_string())),
        };

        let mut session_info = self.user_session_storage
            .get_session_info(user_public_key.clone(), session_id.clone())
            .await?
            .ok_or(SignerError::InvalidUserState("Session state is not SigningRound1".to_string()))?;

        assert_eq!(session_info.message_hash, request.signing_package.message().clone());

        match session_info.state {
            SignerUserSessionState::SigningRound1 {
                nonces,
            } => {
                let tweak_key_package = match session_info.tweak.clone() {
                    Some(tweak) => key_package.clone().tweak(Some(tweak.to_vec())),
                    None => key_package.clone(),
                };
                let signature_share =
                    frost_secp256k1_tr::round2::sign(&request.signing_package, &nonces, &tweak_key_package)
                        .map_err(|e| SignerError::Internal(format!("Sign round2 failed: {e}")))?;
                
                session_info.state = SignerUserSessionState::SigningRound2 {
                    signature_share,
                };

                self.user_session_storage
                    .set_session_info(
                        user_public_key.clone(),
                        session_id.clone(),
                        session_info,
                    )
                    .await?;

                Ok(SignRound2Response {
                    session_id,
                    signature_share,
                })
            }
            _ => {
                Err(SignerError::InvalidUserState(
                    "User session state is not SigningRound1".to_string(),
                ))
            }
        }
    }
}
