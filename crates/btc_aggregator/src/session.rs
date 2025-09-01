use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use frost_secp256k1::keys::KeyPackage;
use frost_secp256k1::round1::{SigningCommitments, SigningNonces};
use frost_secp256k1::round2::SignatureShare;
use frost_secp256k1::{keys::{dkg::{round1, round2}, PublicKeyPackage, SigningShare}, Identifier, Signature};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionState {
    DkgRound1,
    DkgRound2,
    DkgFinalization,
    DkgComplete,
    NonceGeneration,
    Signing,
    SignatureAggregation,
    Completed,
    Failed(String),
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonceShare {
    pub participant_id: Identifier,
    pub commitment: SigningCommitments, 
    pub secret: SigningNonces,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialSignature {
    pub participant_id: Identifier,
    pub signature: SignatureShare,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningSession {
    pub id: String,
    pub state: SessionState,
    pub message: Option<Vec<u8>>,
    pub threshold: u32,
    pub total_participants: u32,
    pub participants: Vec<Identifier>,
    pub created_at: SystemTime,
    pub timeout: Duration,

    pub dkg_round1_packages: BTreeMap<Identifier, round1::Package>,
    pub dkg_round2_packages: BTreeMap<Identifier, round2::Package>,
    pub key_packages: BTreeMap<Identifier, KeyPackage>, 
    pub key_shares: BTreeMap<Identifier, SigningShare>, 
    pub public_key_package: Option<PublicKeyPackage>,

    pub nonce_shares: BTreeMap<Identifier, NonceShare>,
    pub signing_commitments: Option<SigningCommitments>,
    pub partial_signatures: BTreeMap<Identifier, PartialSignature>,
    pub final_signature: Option<Signature>,
}

impl SigningSession {
    pub fn new(
        id: String,
        threshold: u32,
        total_participants: u32,
        participants: Vec<Identifier>,
        timeout: Duration
    ) -> Self {
        Self {
            id,
            state: SessionState::DkgRound1,
            message: None,
            threshold,
            total_participants,
            participants,
            created_at: SystemTime::now(),
            timeout,
            dkg_round1_packages: BTreeMap::new(),
            dkg_round2_packages: BTreeMap::new(),
            key_packages: BTreeMap::new(),
            key_shares: BTreeMap::new(),
            public_key_package: None,
            nonce_shares: BTreeMap::new(),
            signing_commitments: None,
            partial_signatures: BTreeMap::new(),
            final_signature: None,
        }
    }

    pub fn new_for_signing(
        id: String,
        message: Vec<u8>,
        threshold: u32,
        total_participants: u32,
        participants: Vec<Identifier>,
        public_key_package: PublicKeyPackage,
        timeout: Duration,
    ) -> Self {
        Self {
            id,
            state: SessionState::NonceGeneration,
            message: Some(message),
            threshold,
            total_participants,
            participants,
            created_at: SystemTime::now(),
            timeout,
            dkg_round1_packages: BTreeMap::new(),
            dkg_round2_packages: BTreeMap::new(),
            key_packages: BTreeMap::new(),
            key_shares: BTreeMap::new(),
            public_key_package: Some(public_key_package),
            nonce_shares: BTreeMap::new(),
            signing_commitments: None,
            partial_signatures: BTreeMap::new(),
            final_signature: None,
        }
    }

    pub fn add_dkg_round1_package(&mut self, participant_id: Identifier, package: round1::Package) -> bool {
        if !matches!(self.state, SessionState::DkgRound1) {
            return false;
        }

        if !self.participants.contains(&participant_id) {
            return false;
        }

        self.dkg_round1_packages.insert(participant_id, package);

        // Меняем состояние только когда все участники добавили пакеты
        if self.dkg_round1_packages.len() == self.participants.len() {
            self.state = SessionState::DkgRound2;
        }

        true
    }

    pub fn add_dkg_round2_package(&mut self, participant_id: Identifier, package: round2::Package) -> bool {
        if !matches!(self.state, SessionState::DkgRound2) {
            return false;
        }

        if !self.participants.contains(&participant_id) {
            return false;
        }

        self.dkg_round2_packages.insert(participant_id, package);

        // Меняем состояние только когда все участники добавили пакеты
        if self.dkg_round2_packages.len() == self.participants.len() {
            self.state = SessionState::DkgFinalization;
        }

        true
    }

    pub fn finalize_dkg(&mut self, key_packages: BTreeMap<Identifier, KeyPackage>, public_key_package: PublicKeyPackage) {
        self.key_shares = key_packages.iter()
            .map(|(id, kp)| (id.clone(), kp.signing_share().clone()))
            .collect();
        self.key_packages = key_packages;
        self.public_key_package = Some(public_key_package);
        self.state = SessionState::DkgComplete;
    }

    pub fn add_nonce_share(&mut self, share: NonceShare) -> bool {
        if !matches!(self.state, SessionState::NonceGeneration) {
            return false;
        }

        if !self.participants.contains(&share.participant_id) {
            return false;
        }

        self.nonce_shares.insert(share.participant_id.clone(), share);

        if self.nonce_shares.len() >= self.threshold as usize {
            self.state = SessionState::Signing;
        }

        true
    }

    pub fn set_signing_commitments(&mut self, commitments: SigningCommitments) {
        self.signing_commitments = Some(commitments);
    }

    pub fn add_partial_signature(&mut self, sig: PartialSignature) -> bool {
        if !matches!(self.state, SessionState::Signing | SessionState::SignatureAggregation) {
            return false;
        }

        if !self.participants.contains(&sig.participant_id) {
            return false;
        }

        self.partial_signatures.insert(sig.participant_id.clone(), sig);

        true
    }

    pub fn complete_signing(&mut self, final_sig: Signature) {
        self.final_signature = Some(final_sig);
        self.state = SessionState::Completed;
    }

    pub fn is_expired(&self) -> bool {
        if let Ok(elapsed) = SystemTime::now().duration_since(self.created_at) {
            elapsed > self.timeout
        } else {
            false
        }
    }

    pub fn get_progress(&self) -> f64 {
        match self.state {
            SessionState::DkgRound1 => {
                self.dkg_round1_packages.len() as f64 / self.threshold as f64 * 0.25
            },
            SessionState::DkgRound2 => {
                0.25 + (self.dkg_round2_packages.len() as f64 / self.threshold as f64 * 0.25)
            },
            SessionState::DkgFinalization => 0.5,
            SessionState::DkgComplete => 0.6,
            SessionState::NonceGeneration => {
                0.6 + (self.nonce_shares.len() as f64 / self.threshold as f64 * 0.2)
            },
            SessionState::Signing => 0.8,
            SessionState::SignatureAggregation => {
                0.8 + (self.partial_signatures.len() as f64 / self.threshold as f64 * 0.2)
            },
            SessionState::Completed => 1.0,
            SessionState::Failed(_) | SessionState::Timeout => 0.0,
        }
    }

    pub fn fail_session(&mut self, reason: String) {
        self.state = SessionState::Failed(reason);
    }

    pub fn timeout_session(&mut self) {
        self.state = SessionState::Timeout;
    }
}