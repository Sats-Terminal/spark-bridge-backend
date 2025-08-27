use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use serde::{Deserialize, Serialize};
use btc_signer::{
    ParticipantId, KeyShare, PublicKeyPackage, NonceShare, PartialSignature,
    SigningCommitments, DkgRound1Package, DkgRound2Package, FrostSignature
};

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
pub struct SigningSession {
    pub id: String,
    pub state: SessionState,
    pub message: Option<Vec<u8>>,
    pub threshold: u32,
    pub total_participants: u32,
    pub participants: Vec<ParticipantId>,
    pub created_at: SystemTime,
    pub timeout: Duration,

    pub dkg_round1_packages: HashMap<ParticipantId, DkgRound1Package>,
    pub dkg_round2_packages: HashMap<ParticipantId, DkgRound2Package>,
    pub key_shares: HashMap<ParticipantId, KeyShare>,
    pub public_key_package: Option<PublicKeyPackage>,

    pub nonce_shares: HashMap<ParticipantId, NonceShare>,
    pub signing_commitments: Option<SigningCommitments>,
    pub partial_signatures: HashMap<ParticipantId, PartialSignature>,
    pub final_signature: Option<FrostSignature>,
}

impl SigningSession {
    pub fn new(
        id: String,
        threshold: u32,
        total_participants: u32,
        participants: Vec<ParticipantId>,
        timeout: Duration,
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
            dkg_round1_packages: HashMap::new(),
            dkg_round2_packages: HashMap::new(),
            key_shares: HashMap::new(),
            public_key_package: None,
            nonce_shares: HashMap::new(),
            signing_commitments: None,
            partial_signatures: HashMap::new(),
            final_signature: None,
        }
    }

    pub fn new_for_signing(
        id: String,
        message: Vec<u8>,
        threshold: u32,
        total_participants: u32,
        participants: Vec<ParticipantId>,
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
            dkg_round1_packages: HashMap::new(),
            dkg_round2_packages: HashMap::new(),
            key_shares: HashMap::new(),
            public_key_package: Some(public_key_package),
            nonce_shares: HashMap::new(),
            signing_commitments: None,
            partial_signatures: HashMap::new(),
            final_signature: None,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().unwrap_or_default() > self.timeout
    }

    pub fn can_proceed_to_dkg_round2(&self) -> bool {
        matches!(self.state, SessionState::DkgRound1) &&
            self.dkg_round1_packages.len() >= self.threshold as usize
    }

    pub fn can_finalize_dkg(&self) -> bool {
        matches!(self.state, SessionState::DkgRound2) &&
            self.dkg_round2_packages.len() >= self.threshold as usize
    }

    pub fn can_proceed_to_signing(&self) -> bool {
        matches!(self.state, SessionState::NonceGeneration) &&
            self.nonce_shares.len() >= self.threshold as usize
    }

    pub fn can_aggregate_signatures(&self) -> bool {
        matches!(self.state, SessionState::Signing) &&
            self.partial_signatures.len() >= self.threshold as usize
    }

    pub fn add_dkg_round1_package(&mut self, package: DkgRound1Package) -> bool {
        if !matches!(self.state, SessionState::DkgRound1) {
            return false;
        }

        if !self.participants.contains(&package.participant_id) {
            return false;
        }

        self.dkg_round1_packages.insert(package.participant_id.clone(), package);

        if self.can_proceed_to_dkg_round2() {
            self.state = SessionState::DkgRound2;
        }

        true
    }

    pub fn add_dkg_round2_package(&mut self, package: DkgRound2Package) -> bool {
        if !matches!(self.state, SessionState::DkgRound2) {
            return false;
        }

        if !self.participants.contains(&package.participant_id) {
            return false;
        }

        self.dkg_round2_packages.insert(package.participant_id.clone(), package);

        if self.can_finalize_dkg() {
            self.state = SessionState::DkgFinalization;
        }

        true
    }

    pub fn finalize_dkg(&mut self, key_shares: HashMap<ParticipantId, KeyShare>, public_key_package: PublicKeyPackage) {
        self.key_shares = key_shares;
        self.public_key_package = Some(public_key_package);
        self.state = SessionState::DkgComplete;
    }

    pub fn start_signing(&mut self, message: Vec<u8>) -> bool {
        if !matches!(self.state, SessionState::DkgComplete) {
            return false;
        }

        self.message = Some(message);
        self.state = SessionState::NonceGeneration;
        true
    }

    pub fn add_nonce_share(&mut self, nonce_share: NonceShare) -> bool {
        if !matches!(self.state, SessionState::NonceGeneration) {
            return false;
        }

        if !self.participants.contains(&nonce_share.participant_id) {
            return false;
        }

        self.nonce_shares.insert(nonce_share.participant_id.clone(), nonce_share);

        if self.can_proceed_to_signing() {
            let commitments = self.nonce_shares.iter()
                .map(|(id, share)| (id.clone(), share.commitment.clone()))
                .collect();

            self.signing_commitments = Some(SigningCommitments { commitments });
            self.state = SessionState::Signing;
        }

        true
    }

    pub fn add_partial_signature(&mut self, partial_sig: PartialSignature) -> bool {
        if !matches!(self.state, SessionState::Signing) {
            return false;
        }

        if !self.participants.contains(&partial_sig.participant_id) {
            return false;
        }

        self.partial_signatures.insert(partial_sig.participant_id.clone(), partial_sig);

        if self.can_aggregate_signatures() {
            self.state = SessionState::SignatureAggregation;
        }

        true
    }

    pub fn complete_signing(&mut self, signature: FrostSignature) {
        self.final_signature = Some(signature);
        self.state = SessionState::Completed;
    }

    pub fn fail(&mut self, reason: String) {
        self.state = SessionState::Failed(reason);
    }

    pub fn mark_timeout(&mut self) {
        self.state = SessionState::Timeout;
    }
}