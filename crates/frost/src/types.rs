use bitcoin::secp256k1::PublicKey;
use frost_secp256k1_tr::{
    Identifier, Signature, SigningPackage,
    keys::{
        KeyPackage, PublicKeyPackage,
        dkg::{round1, round2},
    },
    round1::{SigningCommitments, SigningNonces},
    round2::SignatureShare,
};
use lrc20::token_transaction::TokenTransaction;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub type RuneId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum MusigId {
    User {
        user_public_key: PublicKey,
        rune_id: RuneId,
    },
    Issuer {
        issuer_public_key: PublicKey,
        rune_id: RuneId,
    },
}

impl MusigId {
    pub fn get_public_key(&self) -> PublicKey {
        match self {
            MusigId::User { user_public_key, .. } => user_public_key.clone(),
            MusigId::Issuer { issuer_public_key, .. } => issuer_public_key.clone(),
        }
    }

    pub fn get_rune_id(&self) -> String {
        match self {
            MusigId::User { rune_id, .. } => rune_id.clone(),
            MusigId::Issuer { rune_id, .. } => rune_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound1Request {
    pub musig_id: MusigId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound1Response {
    pub round1_package: round1::Package,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound2Request {
    pub musig_id: MusigId,
    pub round1_packages: BTreeMap<Identifier, round1::Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound2Response {
    pub round2_packages: BTreeMap<Identifier, round2::Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgFinalizeRequest {
    pub musig_id: MusigId,
    pub round1_packages: BTreeMap<Identifier, round1::Package>,
    pub round2_packages: BTreeMap<Identifier, round2::Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgFinalizeResponse {
    pub public_key_package: PublicKeyPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound1Request {
    pub musig_id: MusigId,
    pub session_id: Uuid,
    pub metadata: SigningMetadata,
    pub message_hash: Vec<u8>,
    pub tweak: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound1Response {
    pub commitments: SigningCommitments,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound2Request {
    pub musig_id: MusigId,
    pub session_id: Uuid,
    pub signing_package: SigningPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound2Response {
    pub signature_share: SignatureShare,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregatorDkgState {
    DkgRound1 {
        round1_packages: BTreeMap<Identifier, round1::Package>,
    },
    DkgRound2 {
        round1_packages: BTreeMap<Identifier, round1::Package>,
        round2_packages: BTreeMap<Identifier, BTreeMap<Identifier, round2::Package>>,
    },
    DkgFinalized {
        public_key_package: PublicKeyPackage,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatorMusigIdData {
    pub dkg_state: AggregatorDkgState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregatorSignState {
    SigningRound1 { signing_package: SigningPackage },
    SigningRound2 { signature: Signature },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatorSignData {
    pub tweak: Option<Vec<u8>>,
    pub message_hash: Vec<u8>,
    pub metadata: SigningMetadata,
    pub sign_state: AggregatorSignState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignerDkgState {
    DkgRound1 {
        round1_secret_package: round1::SecretPackage,
    },
    DkgRound2 {
        round2_secret_package: round2::SecretPackage,
        round1_packages: BTreeMap<Identifier, round1::Package>,
    },
    DkgFinalized {
        key_package: KeyPackage,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerMusigIdData {
    pub dkg_state: SignerDkgState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignerSignState {
    SigningRound1 { nonces: SigningNonces },
    SigningRound2 { signature_share: SignatureShare },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerSignData {
    pub tweak: Option<Vec<u8>>,
    pub message_hash: Vec<u8>,
    pub metadata: SigningMetadata,
    pub sign_state: SignerSignState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningMetadata {
    pub token_transaction_metadata: TokenTransactionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenTransactionMetadata {
    PartialCreateToken { token_transaction: TokenTransaction },
    FinalCreateToken { token_transaction: TokenTransaction },
    PartialMintToken { token_transaction: TokenTransaction },
    FinalMintToken { token_transaction: TokenTransaction },
}
