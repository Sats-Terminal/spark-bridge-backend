use frost_secp256k1_tr::aggregate;
use frost_secp256k1_tr::keys::dkg::{round1, round2};
use frost_secp256k1_tr::keys::{KeyPackage, PublicKeyPackage};
use frost_secp256k1_tr::round1::SigningCommitments;
use frost_secp256k1_tr::round2::SignatureShare;
use frost_secp256k1_tr::{Identifier, Signature, SigningPackage};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AggregatorError {
    #[error("User not found: {user_id}")]
    UserNotFound { user_id: String },

    #[error("Verifier not found: {id}")]
    VerifierNotFound { id: String },

    #[error("DKG process failed: {reason}")]
    DkgFailed { reason: String },

    #[error("Signing process failed: {reason}")]
    SigningFailed { reason: String },

    #[error("Insufficient verifiers: got {got}, need {need}")]
    InsufficientVerifiers { got: usize, need: usize },

    #[error("Network error: {0}")]
    Network(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, AggregatorError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSpecificInfo {
    pub user_id: String,
    pub public_key_package: Option<PublicKeyPackage>,
    pub key_package: Option<KeyPackage>,
    pub created_at: std::time::SystemTime,
}

impl UserSpecificInfo {
    pub fn new(user_id: String) -> Self {
        Self {
            user_id,
            public_key_package: None,
            key_package: None,
            created_at: std::time::SystemTime::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound1Request {
    pub session_id: String,
    pub user_id: String,
    pub participant_ids: Vec<Identifier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound1Response {
    pub session_id: String,
    pub participant_id: Identifier,
    pub round1_package: round1::Package,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound2Request {
    pub session_id: String,
    pub user_id: String,
    pub round1_packages: BTreeMap<Identifier, round1::Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound2Response {
    pub session_id: String,
    pub participant_id: Identifier,
    pub round2_packages: BTreeMap<Identifier, round2::Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgFinalizeRequest {
    pub session_id: String,
    pub user_id: String,
    pub round1_packages: BTreeMap<Identifier, round1::Package>,
    pub round2_packages: BTreeMap<Identifier, round2::Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgFinalizeResponse {
    pub session_id: String,
    pub participant_id: Identifier,
    pub key_package: KeyPackage,
    pub public_key_package: PublicKeyPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound1Request {
    pub session_id: String,
    pub user_id: String,
    pub message: Vec<u8>,
    pub participant_ids: Vec<Identifier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound1Response {
    pub session_id: String,
    pub participant_id: Identifier,
    pub commitments: SigningCommitments, // Only commitment
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound2Request {
    pub session_id: String,
    pub user_id: String,
    pub message: Vec<u8>,
    pub commitments: BTreeMap<Identifier, SigningCommitments>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound2Response {
    pub session_id: String,
    pub participant_id: Identifier,
    pub signature_share: SignatureShare,
}

#[async_trait::async_trait]
pub trait SignerClient: Send + Sync {
    async fn dkg_round_1(&self, request: DkgRound1Request) -> Result<DkgRound1Response>;

    async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response>;

    async fn dkg_finalize(&self, request: DkgFinalizeRequest) -> Result<DkgFinalizeResponse>;

    async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response>;

    async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response>;

    async fn get_participant_id(&self) -> Identifier;
}

pub struct Aggregator {
    verifiers: HashMap<Identifier, Arc<dyn SignerClient>>,
    users: Arc<RwLock<HashMap<String, UserSpecificInfo>>>,
    threshold: u16,
    total_participants: u16,
}

impl Aggregator {
    pub fn new(
        verifiers: HashMap<Identifier, Arc<dyn SignerClient>>,
        threshold: u16,
        total_participants: u16,
    ) -> Result<Self> {
        if verifiers.len() != total_participants as usize {
            return Err(AggregatorError::InsufficientVerifiers {
                got: verifiers.len(),
                need: total_participants as usize,
            });
        }

        Ok(Self {
            verifiers,
            users: Arc::new(RwLock::new(HashMap::new())),
            threshold,
            total_participants,
        })
    }

    pub async fn check_user_id(&self, user_id: &str) -> bool {
        let users = self.users.read().await;
        users.contains_key(user_id)
    }
    pub async fn create_user(&self, user_id: &str) -> Result<()> {
        let mut users = self.users.write().await;
        if users.contains_key(user_id) {
            return Ok(());
        }
        users.insert(user_id.to_string(), UserSpecificInfo::new(user_id.to_string()));
        Ok(())
    }
    pub async fn run_dkg_flow(&self, user_id: &str) -> Result<PublicKeyPackage> {

        if !self.check_user_id(user_id).await {
            self.create_user(user_id).await?;
        }

        let session_id = Uuid::new_v4().to_string();
        let participant_ids: Vec<Identifier> = self.verifiers.keys().cloned().collect();

        let mut round1_responses = Vec::new();
        for (verifier_id, client) in &self.verifiers {
            let request = DkgRound1Request {
                session_id: session_id.clone(),
                user_id: user_id.to_string(),
                participant_ids: participant_ids.clone(),
            };

            let response = client.dkg_round_1(request).await
                .map_err(|e| AggregatorError::DkgFailed {
                    reason: format!("Round 1 failed for verifier {:#?}: {}", verifier_id.serialize().as_slice(), e)
                })?;

            round1_responses.push(response);
        }

        let round1_packages: BTreeMap<Identifier, round1::Package> = round1_responses
            .into_iter()
            .map(|resp| (resp.participant_id, resp.round1_package))
            .collect();

        let mut round2_responses = Vec::new();
        for (verifier_id, client) in &self.verifiers {
            let request = DkgRound2Request {
                session_id: session_id.clone(),
                user_id: user_id.to_string(),
                round1_packages: round1_packages.clone(),
            };

            let response = client.dkg_round_2(request).await
                .map_err(|e| AggregatorError::DkgFailed {
                    reason: format!("Round 2 failed for verifier {:#?}: {}", verifier_id.serialize().as_slice(), e)
                })?;

            round2_responses.push(response);
        }

        let mut all_round2_packages = BTreeMap::new();
        for response in round2_responses {
            all_round2_packages.extend(response.round2_packages);
        }

        let mut finalize_responses = Vec::new();
        for (verifier_id, client) in &self.verifiers {
            let request = DkgFinalizeRequest {
                session_id: session_id.clone(),
                user_id: user_id.to_string(),
                round1_packages: round1_packages.clone(),
                round2_packages: all_round2_packages.clone(),
            };

            let response = client.dkg_finalize(request).await
                .map_err(|e| AggregatorError::DkgFailed {
                    reason: format!("Finalization failed for verifier {:#?}: {}", verifier_id.serialize().as_slice(), e)
                })?;

            finalize_responses.push(response);
        }

        let public_key_package = finalize_responses[0].public_key_package.clone();

        {
            let mut users = self.users.write().await;
            if let Some(user_info) = users.get_mut(user_id) {
                user_info.public_key_package = Some(public_key_package.clone());
            }
        }

        Ok(public_key_package)
    }

    pub async fn run_signing_flow(&self, user_id: &str, message: &[u8]) -> Result<Signature> {
        let public_key_package = {
            let users = self.users.read().await;
            let user_info = users.get(user_id)
                .ok_or_else(|| AggregatorError::UserNotFound { user_id: user_id.to_string() })?;

            user_info.public_key_package.clone()
                .ok_or_else(|| AggregatorError::Internal("User has no public key package".to_string()))?
        };

        let session_id = Uuid::new_v4().to_string();
        let participant_ids: Vec<Identifier> = self.verifiers.keys().cloned().collect();

        let mut sign_round1_responses = Vec::new();
        for (verifier_id, client) in &self.verifiers {
            let request = SignRound1Request {
                session_id: session_id.clone(),
                user_id: user_id.to_string(),
                message: message.to_vec(),
                participant_ids: participant_ids.clone(),
            };

            let response = client.sign_round_1(request).await
                .map_err(|e| AggregatorError::SigningFailed {
                    reason: format!("Sign round 1 failed for verifier {:#?}: {}", verifier_id.serialize().as_slice(), e)
                })?;

            sign_round1_responses.push(response);
        }

        let commitments: BTreeMap<Identifier, SigningCommitments> = sign_round1_responses
            .into_iter()
            .map(|resp| (resp.participant_id, resp.commitments))
            .collect();

        let mut signature_shares = Vec::new();
        for (verifier_id, client) in &self.verifiers {
            let request = SignRound2Request {
                session_id: session_id.clone(),
                user_id: user_id.to_string(),
                message: message.to_vec(),
                commitments: commitments.clone(),
            };

            let response = client.sign_round_2(request).await
                .map_err(|e| AggregatorError::SigningFailed {
                    reason: format!("Sign round 2 failed for verifier {:#?}: {}", verifier_id.serialize().as_slice(), e)
                })?;

            signature_shares.push((response.participant_id, response.signature_share));
        }

        signature_shares.truncate(self.threshold as usize);

        let signing_package = SigningPackage::new(commitments, message);

        let signature_shares_map: BTreeMap<Identifier, SignatureShare> = signature_shares
            .into_iter()
            .collect();

        let signature = aggregate(&signing_package, &signature_shares_map, &public_key_package)
            .map_err(|e| AggregatorError::SigningFailed {
                reason: format!("Signature aggregation failed: {:?}", e)
            })?;

        Ok(signature)
    }

    pub async fn get_public_key_package(&self, user_id: &str) -> Result<PublicKeyPackage> {
        let users = self.users.read().await;
        let user_info = users.get(user_id)
            .ok_or_else(|| AggregatorError::UserNotFound { user_id: user_id.to_string() })?;

        user_info.public_key_package.clone()
            .ok_or_else(|| AggregatorError::Internal("User has no public key package".to_string()))
    }

    pub async fn list_users(&self) -> Vec<String> {
        let users = self.users.read().await;
        users.keys().cloned().collect()
    }

    pub async fn remove_user(&self, user_id: &str) -> Result<()> {
        let mut users = self.users.write().await;
        users.remove(user_id)
            .ok_or_else(|| AggregatorError::UserNotFound { user_id: user_id.to_string() })?;
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait AggregatorTrait {
    async fn check_user_id(&self, user_id: &str) -> bool;
    async fn run_dkg_flow(&self, user_id: &str) -> Result<PublicKeyPackage>;
    async fn run_signing_flow(&self, user_id: &str, message: &[u8]) -> Result<Signature>;
    async fn get_public_key_package(&self, user_id: &str) -> Result<PublicKeyPackage>;
}

#[async_trait::async_trait]
impl AggregatorTrait for Aggregator {
    async fn check_user_id(&self, user_id: &str) -> bool {
        self.check_user_id(user_id).await
    }

    async fn run_dkg_flow(&self, user_id: &str) -> Result<PublicKeyPackage> {
        self.run_dkg_flow(user_id).await
    }

    async fn run_signing_flow(&self, user_id: &str, message: &[u8]) -> Result<Signature> {
        self.run_signing_flow(user_id, message).await
    }

    async fn get_public_key_package(&self, user_id: &str) -> Result<PublicKeyPackage> {
        self.get_public_key_package(user_id).await
    }
}