use std::collections::{BTreeMap, HashMap};
use std::ops::Deref;
use std::sync::Arc;
use frost_secp256k1::{Identifier, Signature, SigningPackage};
use frost_secp256k1::keys::PublicKeyPackage;
use frost_secp256k1::round1::SigningNonces;
use frost_secp256k1::round2::SignatureShare;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{info, warn};
use uuid::Uuid;

use btc_signer::api::Signer;

use frost_secp256k1::keys::dkg::{round1, round2};
use frost_secp256k1::aggregate;
use crate::{
    session::{SigningSession, SessionState, NonceShare, PartialSignature},
    config::AggregatorConfig,
    errors::{Result, AggregatorError},
};

pub struct FrostAggregator {
    config: AggregatorConfig,
    pub signers: Arc<RwLock<HashMap<Identifier, Arc<dyn Signer>>>>,
    pub sessions: Arc<RwLock<HashMap<String, SigningSession>>>,
    cleanup_task: Option<tokio::task::JoinHandle<()>>,
}

impl FrostAggregator {
    pub fn new(config: AggregatorConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
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
                id: String::try_from(participant_id.serialize()).unwrap(),
            });
        }

        signers.insert(participant_id.clone(), signer);
        info!("Added signer: {:#?}", participant_id.serialize());
        Ok(())
    }

    pub async fn remove_signer(&self, participant_id: &Identifier) -> Result<()> {
        let mut signers = self.signers.write().await;
        signers.remove(participant_id).ok_or_else(|| AggregatorError::ParticipantNotFound {
            id: String::try_from(participant_id.serialize()).unwrap(),
        })?;
        info!("Removed signer: {:#?}", participant_id.serialize());
        Ok(())
    }

    pub async fn start_dkg_session(&self, participants: Vec<Identifier>) -> Result<String> {
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
                    id: String::try_from(participant.serialize()).unwrap(),
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

    pub async fn process_dkg_round1(&self, session_id: &str) -> Result<Vec<round1::Package>> {
        let participants = {
            let sessions = self.sessions.read().await;
            let session = sessions.get(session_id)
                .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;
            if !matches!(session.state, SessionState::DkgRound1) {
                return Err(AggregatorError::InvalidSessionState { state: format!("{:?}", session.state) });
            }
            session.participants.clone()
        };

        let signers = self.signers.read().await;
        let mut packages = Vec::new();

        for participant_id in &participants {
            if let Some(signer) = signers.get(participant_id) {
                
                let mut signer_mut = Arc::clone(signer);
                let package = Arc::get_mut(&mut signer_mut)
                    .ok_or_else(|| AggregatorError::Internal("Failed to get mutable signer".to_string()))?
                    .dkg_round_1().await?;

                packages.push(package.clone());

                let mut sessions = self.sessions.write().await;
                if let Some(session) = sessions.get_mut(session_id) {
                    session.add_dkg_round1_package(participant_id.clone(), package);
                }
            }
        }

        Ok(packages)
    }

    pub async fn process_dkg_round2(
        &self,
        session_id: &str,
    ) -> Result<BTreeMap<Identifier, round2::Package>> {
        let (participants, round1_packages) = {
            let sessions = self.sessions.read().await;
            let session = sessions.get(session_id)
                .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;
            if !matches!(session.state, SessionState::DkgRound2) {
                return Err(AggregatorError::InvalidSessionState { state: format!("{:?}", session.state) });
            }
            (session.participants.clone(), session.dkg_round1_packages.clone())
        };

        let signers = self.signers.read().await;
        let mut round2_results = BTreeMap::new();

        for participant_id in &participants {
            if let Some(signer) = signers.get(participant_id) {
                let mut signer_mut = Arc::clone(signer);
                let round2_map = Arc::get_mut(&mut signer_mut)
                    .ok_or_else(|| AggregatorError::Internal("Failed to get mutable signer".to_string()))?
                    .dkg_round_2(&round1_packages).await?;

                round2_results.extend(round2_map.clone());

                let mut sessions = self.sessions.write().await;
                if let Some(session) = sessions.get_mut(session_id) {
                    if let Some(package) = round2_map.get(participant_id) {
                        session.add_dkg_round2_package(participant_id.clone(), package.clone());
                    }
                }
            }
        }

        Ok(round2_results)
    }

    pub async fn finalize_dkg(&self, session_id: &str) -> Result<PublicKeyPackage> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;

        if !matches!(session.state, SessionState::DkgFinalization) {
            return Err(AggregatorError::InvalidSessionState { state: format!("{:?}", session.state) });
        }

        let signers = self.signers.read().await;
        let mut key_packages = BTreeMap::new();
        let mut public_key_package = None;

        for participant_id in &session.participants {
            if let Some(signer) = signers.get(participant_id) {
                let mut signer_mut = Arc::clone(signer);
                let (key_package, pub_key_package) = Arc::get_mut(&mut signer_mut)
                    .ok_or_else(|| AggregatorError::Internal("Failed to get mutable signer".to_string()))?
                    .finalize_dkg(
                        &session.dkg_round1_packages,
                        &session.dkg_round2_packages,
                    ).await?;

                key_packages.insert(participant_id.clone(), key_package);
                public_key_package = Some(pub_key_package);
            }
        }

        let public_key_package = public_key_package
            .ok_or_else(|| AggregatorError::Internal("No public key package generated".to_string()))?;

        session.finalize_dkg(key_packages, public_key_package.clone());

        Ok(public_key_package)
    }

    pub async fn start_signing_session(
        &self,
        message: Vec<u8>,
        participants: Vec<Identifier>,
        public_key_package: PublicKeyPackage,
    ) -> Result<String> {
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

    pub async fn collect_nonce_shares(&self, session_id: &str) -> Result<Vec<SigningNonces>> {
        let participants = {
            let sessions = self.sessions.read().await;
            let session = sessions.get(session_id)
                .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;
            if !matches!(session.state, SessionState::NonceGeneration) {
                return Err(AggregatorError::InvalidSessionState { state: format!("{:?}", session.state) });
            }
            session.participants.clone()
        };

        let signers = self.signers.read().await;
        let mut nonces = Vec::new();

        for participant_id in &participants {
            if let Some(signer) = signers.get(participant_id) {
                let mut signer_mut = Arc::clone(signer);
                let nonce = Arc::get_mut(&mut signer_mut)
                    .ok_or_else(|| AggregatorError::Internal("Failed to get mutable signer".to_string()))?
                    .generate_nonce_share().await?;

                nonces.push(nonce.clone());

                let nonce_share = NonceShare {
                    participant_id: participant_id.clone(),
                    commitment: nonce.commitments().deref().clone(),
                    secret: nonce,
                };

                let mut sessions = self.sessions.write().await;
                if let Some(session) = sessions.get_mut(session_id) {
                    session.add_nonce_share(nonce_share);
                }
            }
        }

        Ok(nonces)
    }

    pub async fn collect_partial_signatures(&self, session_id: &str) -> Result<Vec<SignatureShare>> {
        let (participants, message, public_key_package, nonces, nonce_shares) = {
            let sessions = self.sessions.read().await;
            let session = sessions.get(session_id)
                .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;
            if !matches!(session.state, SessionState::Signing) {
                return Err(AggregatorError::InvalidSessionState { state: format!("{:?}", session.state) });
            }
            let message = session.message.as_ref().ok_or_else(|| AggregatorError::Internal("No message".to_string()))?;
            let public_key_package = session.public_key_package.as_ref().ok_or_else(|| AggregatorError::Internal("No public key package".to_string()))?;

            // let nonces: Vec<SigningNonces> = session.nonce_shares.values()
            //     .map(|ns| ns.secret.clone())
            //     .collect();
            //
            // (session.participants.clone(), message.clone(), public_key_package.clone(), nonces)
            let nonces: Vec<SigningNonces> = session.nonce_shares.values()
                .map(|ns| ns.secret.clone())
                .collect();

            (
                session.participants.clone(),
                message.clone(),
                public_key_package.clone(),
                nonces,
                session.nonce_shares.clone()  
            )
        };

        // let commitments: BTreeMap<Identifier, _> = session.nonce_share().iter()
        //     .map(|(id, nonce_share)| (id.clone(), nonce_share.commitment.clone()))
        //     .collect();

        let commitments: BTreeMap<Identifier, _> = nonce_shares.iter()
            .map(|(id, nonce_share)| (*id, nonce_share.commitment.clone()))
            .collect();

        let signing_package = SigningPackage::new(
            commitments,
            &message,
        );

        let signers = self.signers.read().await;
        let mut partials = Vec::new();

        for (i, participant_id) in participants.iter().enumerate() {
            if let Some(signer) = signers.get(participant_id) {
                
                let key_package = {
                    let sessions = self.sessions.read().await;
                    let session = sessions.get(session_id).unwrap();
                    session.key_packages.get(participant_id)
                        .ok_or_else(|| AggregatorError::Internal("No key package found".to_string()))?
                        .clone()
                };

                let sig_share = signer.create_partial_signature(
                    &signing_package,
                    &nonces[i],
                    &key_package, 
                ).await?;

                partials.push(sig_share.clone());

                let partial_sig = PartialSignature {
                    participant_id: participant_id.clone(),
                    signature: sig_share,
                };

                let mut sessions = self.sessions.write().await;
                if let Some(session) = sessions.get_mut(session_id) {
                    session.add_partial_signature(partial_sig);
                }
            }
        }

        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.state = SessionState::SignatureAggregation;
        }

        Ok(partials)
    }

    pub async fn aggregate_signatures(&self, session_id: &str, partial_signatures: &[SignatureShare]) -> Result<Signature> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| AggregatorError::SessionNotFound { id: session_id.to_string() })?;

        if !matches!(session.state, SessionState::SignatureAggregation) {
            return Err(AggregatorError::InvalidSessionState { state: format!("{:?}", session.state) });
        }

        if partial_signatures.len() < self.config.threshold as usize {
            return Err(AggregatorError::InsufficientParticipants {
                got: partial_signatures.len(),
                need: self.config.threshold as usize
            });
        }

        let message = session.message.as_ref()
            .ok_or_else(|| AggregatorError::Internal("No message".to_string()))?;
        let public_key_package = session.public_key_package.as_ref()
            .ok_or_else(|| AggregatorError::Internal("No public key package".to_string()))?;

        let commitments: BTreeMap<Identifier, _> = session.nonce_shares.iter()
            .map(|(id, nonce_share)| (id.clone(), nonce_share.commitment.clone()))
            .collect();

        let signing_package = SigningPackage::new(
            commitments,
            message,
        );

        let signature_shares: BTreeMap<Identifier, SignatureShare> = partial_signatures.iter()
            .enumerate()
            .map(|(i, sig)| (session.participants[i].clone(), sig.clone()))
            .collect();

        let aggregated = aggregate(
            &signing_package,
            &signature_shares,
            &public_key_package,
        )
            .map_err(|e| AggregatorError::Internal(format!("Signature aggregation failed: {:?}", e)))?;

        session.complete_signing(aggregated.clone());

        Ok(aggregated)
    }

    pub async fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let mut expired_sessions = Vec::new();

        let now = std::time::SystemTime::now();

        for (session_id, session) in sessions.iter() {
            if let Ok(elapsed) = now.duration_since(session.created_at) {
                if elapsed > session.timeout {
                    expired_sessions.push(session_id.clone());
                }
            }
        }

        for session_id in expired_sessions {
            sessions.remove(&session_id);
            warn!("Removed expired session: {}", session_id);
        }
    }

    pub async fn start_cleanup_task(&mut self) {
        if self.cleanup_task.is_some() {
            return;
        }

        let sessions = Arc::clone(&self.sessions);
        let cleanup_interval = Duration::from_secs(60);

        let handle = tokio::spawn(async move {
            let mut interval = interval(cleanup_interval);

            loop {
                interval.tick().await;

                let mut sessions = sessions.write().await;
                let mut expired_sessions = Vec::new();

                let now = std::time::SystemTime::now();

                for (session_id, session) in sessions.iter() {
                    if let Ok(elapsed) = now.duration_since(session.created_at) {
                        if elapsed > session.timeout {
                            expired_sessions.push(session_id.clone());
                        }
                    }
                }

                for session_id in expired_sessions {
                    sessions.remove(&session_id);
                    warn!("Removed expired session: {}", session_id);
                }
            }
        });

        self.cleanup_task = Some(handle);
    }

    pub async fn stop_cleanup_task(&mut self) {
        if let Some(handle) = self.cleanup_task.take() {
            handle.abort();
        }
    }
}