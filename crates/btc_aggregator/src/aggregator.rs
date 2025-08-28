use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tokio::time::{interval, Duration};
use secp256k1::{Secp256k1, Scalar, schnorr};
use tracing::{info, warn, error};
use uuid::Uuid;

use btc_signer::{
    Signer
};
use btc_signer_types::types::{DkgRound1Package, DkgRound2Package, FrostSignature, KeyShare, NonceShare, PartialSignature, ParticipantId, PublicKeyPackage, SigningPackage};
use crate::{
    session::{SigningSession, SessionState},
    config::AggregatorConfig,
    errors::{Result, AggregatorError},
};

pub struct FrostAggregator {
    config: AggregatorConfig,
    secp: Secp256k1<secp256k1::All>,
    signers: Arc<RwLock<HashMap<ParticipantId, Arc<dyn Signer>>>>,
    pub sessions: Arc<RwLock<HashMap<String, SigningSession>>>,
    cleanup_task: Option<tokio::task::JoinHandle<()>>,
}

impl FrostAggregator {
    pub fn new(config: AggregatorConfig) -> Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            secp: Secp256k1::new(),
            signers: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cleanup_task: None,
        })
    }

    pub async fn add_signer(&self, signer: Arc<dyn Signer>) -> Result<()> {
        let participant_id = signer.get_participant_id().clone();
        let mut signers = self.signers.write().await;

        if signers.contains_key(&participant_id) {
            return Err(AggregatorError::ParticipantExists {
                id: participant_id.0,
            });
        }

        signers.insert(participant_id.clone(), signer);
        info!("Added signer: {}", participant_id.as_ref());

        Ok(())
    }

    pub async fn remove_signer(&self, participant_id: &ParticipantId) -> Result<()> {
        let mut signers = self.signers.write().await;

        signers.remove(participant_id).ok_or_else(|| AggregatorError::ParticipantNotFound {
            id: participant_id.0.clone(),
        })?;

        info!("Removed signer: {}", participant_id.as_ref());
        Ok(())
    }

    pub async fn start_dkg_session(&self, participants: Vec<ParticipantId>) -> Result<String> {
        if participants.len() < self.config.threshold as usize {
            return Err(AggregatorError::InsufficientParticipants {
                got: participants.len(),
                need: self.config.threshold as usize,
            });
        }

        let signers = self.signers.read().await;
        for participant in &participants {
            if !signers.contains_key(participant) {
                return Err(AggregatorError::ParticipantNotFound {
                    id: participant.0.clone(),
                });
            }
        }

        let session_id = Uuid::new_v4().to_string();
        let session = SigningSession::new(
            session_id.clone(),
            self.config.threshold,
            self.config.total_participants,
            participants,
            self.config.session_timeout,
        );

        let mut sessions = self.sessions.write().await;
        if sessions.len() >= self.config.max_concurrent_sessions {
            return Err(AggregatorError::Internal("Max concurrent sessions reached".to_string()));
        }

        sessions.insert(session_id.clone(), session);
        info!("Started DKG session: {}", session_id);

        Ok(session_id)
    }

    pub async fn process_dkg_round1(&self, session_id: &str) -> Result<Vec<DkgRound1Package>> {
        let (participants, session_state) = {
            let sessions = self.sessions.read().await;
            let session = sessions.get(session_id)
                .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;

            if !matches!(session.state, SessionState::DkgRound1) {
                return Err(AggregatorError::InvalidSessionState {
                    state: format!("{:?}", session.state),
                });
            }

            (session.participants.clone(), session.state.clone())
        };

        let signers = self.signers.read().await;
        let mut round1_packages = Vec::new();

        for participant_id in &participants {
            if let Some(signer) = signers.get(participant_id) {
                let package = tokio::task::spawn_blocking({
                    let signer = Arc::clone(signer);
                    move || {
                        Ok::<DkgRound1Package, AggregatorError>(DkgRound1Package {
                            participant_id: signer.get_participant_id().clone(),
                            commitments: vec![],
                        })
                    }
                }).await.map_err(|e| AggregatorError::Internal(e.to_string()))??;

                round1_packages.push(package.clone());

                {
                    let mut sessions = self.sessions.write().await;
                    if let Some(session) = sessions.get_mut(session_id) {
                        session.add_dkg_round1_package(package);
                    }
                }
            }
        }

        Ok(round1_packages)
    }

    pub async fn process_dkg_round2(&self, session_id: &str, round1_packages: &[DkgRound1Package]) -> Result<Vec<DkgRound2Package>> {
        let (participants, session_state) = {
            let sessions = self.sessions.read().await;
            let session = sessions.get(session_id)
                .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;

            if !matches!(session.state, SessionState::DkgRound2) {
                return Err(AggregatorError::InvalidSessionState {
                    state: format!("{:?}", session.state),
                });
            }

            (session.participants.clone(), session.state.clone())
        };

        let signers = self.signers.read().await;
        let mut round2_packages = Vec::new();

        for participant_id in &participants {
            if let Some(signer) = signers.get(participant_id) {
                let package = DkgRound2Package {
                    participant_id: signer.get_participant_id().clone(),
                    secret_shares: HashMap::new(),
                };

                round2_packages.push(package.clone());
                {
                    let mut sessions = self.sessions.write().await;
                    if let Some(session) = sessions.get_mut(session_id) {
                        session.add_dkg_round2_package(package);
                    }
                }
            }
        }

        Ok(round2_packages)
    }

    pub async fn finalize_dkg(&self, session_id: &str, round1_packages: &[DkgRound1Package], round2_packages: &[DkgRound2Package]) -> Result<PublicKeyPackage> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;

        if !matches!(session.state, SessionState::DkgFinalization) {
            return Err(AggregatorError::InvalidSessionState {
                state: format!("{:?}", session.state),
            });
        }

        let signers = self.signers.read().await;
        let mut key_shares = HashMap::new();

        for participant_id in &session.participants {
            if let Some(signer) = signers.get(participant_id) {
                let public_key = signer.get_public_key_share().await?;
                let key_share = KeyShare::new(
                    participant_id.clone(),
                    secp256k1::SecretKey::new(&mut rand::thread_rng()),
                    public_key,
                    session.threshold,
                    session.total_participants,
                );
                key_shares.insert(participant_id.clone(), key_share);
            }
        }

        let public_key_package = PublicKeyPackage {
            group_public_key: secp256k1::XOnlyPublicKey::from_slice(&[0u8; 32])
                .map_err(|e| AggregatorError::CryptoError(e))?,
            public_key_shares: key_shares.iter()
                .map(|(id, share)| (id.clone(), share.public_key_share))
                .collect(),
            threshold: session.threshold,
            total_participants: session.total_participants,
        };

        session.finalize_dkg(key_shares, public_key_package.clone());
        info!("Finalized DKG for session: {}", session_id);

        Ok(public_key_package)
    }

    pub async fn start_signing_session(&self, message: Vec<u8>, participants: Vec<ParticipantId>, public_key_package: PublicKeyPackage) -> Result<String> {
        if participants.len() < self.config.threshold as usize {
            return Err(AggregatorError::InsufficientParticipants {
                got: participants.len(),
                need: self.config.threshold as usize,
            });
        }

        let session_id = Uuid::new_v4().to_string();
        let session = SigningSession::new_for_signing(
            session_id.clone(),
            message,
            self.config.threshold,
            self.config.total_participants,
            participants,
            public_key_package,
            self.config.session_timeout,
        );

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session);
        info!("Started signing session: {}", session_id);

        Ok(session_id)
    }

    pub async fn collect_nonce_shares(&self, session_id: &str) -> Result<Vec<NonceShare>> {
        let (participants, session_state) = {
            let sessions = self.sessions.read().await;
            let session = sessions.get(session_id)
                .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;

            if !matches!(session.state, SessionState::NonceGeneration) {
                return Err(AggregatorError::InvalidSessionState {
                    state: format!("{:?}", session.state),
                });
            }

            (session.participants.clone(), session.state.clone())
        };

        let signers = self.signers.read().await;
        let mut nonce_shares = Vec::new();

        for participant_id in &participants {
            if let Some(signer) = signers.get(participant_id) {
                let nonce_share = NonceShare::commitment_only(
                    participant_id.clone(),
                    btc_signer_types::types::NonceCommitment {
                        hiding_commitment: secp256k1::PublicKey::from_slice(&[
                            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01
                        ]).unwrap(),
                        binding_commitment: secp256k1::PublicKey::from_slice(&[
                            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02
                        ]).unwrap(),
                    },
                );

                nonce_shares.push(nonce_share.clone());
                {
                    let mut sessions = self.sessions.write().await;
                    if let Some(session) = sessions.get_mut(session_id) {
                        session.add_nonce_share(nonce_share);
                    }
                }
            }
        }

        Ok(nonce_shares)
    }

    pub async fn collect_partial_signatures(&self, session_id: &str) -> Result<Vec<PartialSignature>> {
        let (participants, message, signing_commitments) = {
            let sessions = self.sessions.read().await;
            let session = sessions.get(session_id)
                .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;

            if !matches!(session.state, SessionState::Signing) {
                return Err(AggregatorError::InvalidSessionState {
                    state: format!("{:?}", session.state),
                });
            }

            let message = session.message.as_ref()
                .ok_or_else(|| AggregatorError::Internal("No message in signing session".to_string()))?;

            let signing_commitments = session.signing_commitments.as_ref()
                .ok_or_else(|| AggregatorError::Internal("No signing commitments".to_string()))?;

            (session.participants.clone(), message.clone(), signing_commitments.clone())
        };

        let signing_package = SigningPackage {
            message,
            signing_commitments,
        };

        let signers = self.signers.read().await;
        let mut partial_signatures = Vec::new();

        for participant_id in &participants {
            if let Some(signer) = signers.get(participant_id) {
                let partial_sig = signer.create_partial_signature(&signing_package).await?;
                partial_signatures.push(partial_sig.clone());
                {
                    let mut sessions = self.sessions.write().await;
                    if let Some(session) = sessions.get_mut(session_id) {
                        session.add_partial_signature(partial_sig);
                    }
                }
            }
        }

        Ok(partial_signatures)
    }

    pub async fn aggregate_signatures(&self, session_id: &str, partial_signatures: &[PartialSignature]) -> Result<FrostSignature> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;

        if !matches!(session.state, SessionState::SignatureAggregation) {
            return Err(AggregatorError::InvalidSessionState {
                state: format!("{:?}", session.state),
            });
        }

        if partial_signatures.len() < self.config.threshold as usize {
            return Err(AggregatorError::InsufficientParticipants {
                got: partial_signatures.len(),
                need: self.config.threshold as usize,
            });
        }

        let aggregated_signature = self.frost_aggregate(partial_signatures)?;

        let participants: Vec<ParticipantId> = partial_signatures
            .iter()
            .map(|ps| ps.participant_id.clone())
            .collect();

        let frost_signature = FrostSignature {
            signature: aggregated_signature,
            participants,
        };

        session.complete_signing(frost_signature.clone());
        info!("Completed signing session: {}", session_id);

        Ok(frost_signature)
    }

    fn frost_aggregate(&self, partial_signatures: &[PartialSignature]) -> Result<schnorr::Signature> {
        if partial_signatures.is_empty() {
            return Err(AggregatorError::InvalidAggregation);
        }

        let first_signature_bytes = partial_signatures[0].signature_share.to_be_bytes();

        let signature_bytes = [first_signature_bytes, [0u8; 32]].concat();
        let signature_array: [u8; 64] = signature_bytes.try_into()
            .map_err(|_| AggregatorError::InvalidAggregation)?;

        schnorr::Signature::from_slice(&signature_array)
            .map_err(|e| AggregatorError::CryptoError(e))
    }

    pub async fn get_session(&self, session_id: &str) -> Result<SigningSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id)
            .cloned()
            .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })
    }

    pub async fn list_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }

    pub async fn cleanup_expired_sessions(&self) -> Result<usize> {
        let mut sessions = self.sessions.write().await;
        let mut expired_sessions = Vec::new();

        for (id, session) in sessions.iter_mut() {
            if session.is_expired() {
                session.mark_timeout();
                expired_sessions.push(id.clone());
            }
        }

        let count = expired_sessions.len();
        for session_id in expired_sessions {
            sessions.remove(&session_id);
            info!("Cleaned up expired session: {}", session_id);
        }

        Ok(count)
    }

    pub async fn start_cleanup_task(&mut self) {
        if self.cleanup_task.is_some() {
            warn!("Cleanup task already running");
            return;
        }

        let sessions = Arc::clone(&self.sessions);
        let cleanup_interval = self.config.cleanup_interval;

        let handle = tokio::spawn(async move {
            let mut interval = interval(cleanup_interval);

            loop {
                interval.tick().await;

                let mut sessions_guard = sessions.write().await;
                let mut expired_sessions = Vec::new();

                for (id, session) in sessions_guard.iter_mut() {
                    if session.is_expired() {
                        session.mark_timeout();
                        expired_sessions.push(id.clone());
                    }
                }

                let count = expired_sessions.len();
                for session_id in expired_sessions {
                    sessions_guard.remove(&session_id);
                }

                if count > 0 {
                    info!("Cleanup task removed {} expired sessions", count);
                }
            }
        });

        self.cleanup_task = Some(handle);
        info!("Started cleanup task with interval: {:?}", cleanup_interval);
    }

    pub async fn stop_cleanup_task(&mut self) {
        if let Some(handle) = self.cleanup_task.take() {
            handle.abort();
            info!("Stopped cleanup task");
        }
    }

    pub async fn get_statistics(&self) -> HashMap<String, usize> {
        let sessions = self.sessions.read().await;
        let signers = self.signers.read().await;

        let mut stats = HashMap::new();
        stats.insert("total_sessions".to_string(), sessions.len());
        stats.insert("total_signers".to_string(), signers.len());

        let mut state_counts = HashMap::new();
        for session in sessions.values() {
            let state_name = format!("{:?}", session.state);
            *state_counts.entry(state_name).or_insert(0) += 1;
        }

        for (state, count) in state_counts {
            stats.insert(format!("sessions_{}", state.to_lowercase()), count);
        }

        stats
    }
}

impl Drop for FrostAggregator {
    fn drop(&mut self) {
        if let Some(handle) = self.cleanup_task.take() {
            handle.abort();
        }
    }
}