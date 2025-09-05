use std::collections::BTreeMap;
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
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use lrc20::token_transaction::TokenTransaction;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound1Request {
    pub user_public_key: PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound1Response {
    pub round1_package: round1::Package,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound2Request {
    pub user_public_key: PublicKey,
    pub round1_packages: BTreeMap<Identifier, round1::Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound2Response {
    pub round2_packages: BTreeMap<Identifier, round2::Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgFinalizeRequest {
    pub user_public_key: PublicKey,
    pub round1_packages: BTreeMap<Identifier, round1::Package>,
    pub round2_packages: BTreeMap<Identifier, round2::Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgFinalizeResponse {
    pub public_key_package: PublicKeyPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound1Request {
    pub user_public_key: PublicKey,
    pub session_id: Uuid,
    pub metadata: SigningMetadata,
    pub message_hash: Vec<u8>,
    pub tweak: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound1Response {
    pub user_public_key: PublicKey,
    pub session_id: Uuid,
    pub commitments: SigningCommitments, 
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound2Request {
    pub user_public_key: PublicKey,
    pub session_id: Uuid,
    pub signing_package: SigningPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound2Response {
    pub session_id: Uuid,
    pub signature_share: SignatureShare,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregatorUserKeyState {
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
pub struct AggregatorUserKeyInfo {
    pub state: AggregatorUserKeyState
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregatorUserSessionState {
    SigningRound1 {
        signing_package: SigningPackage,
    },
    SigningRound2 {
        signature: Signature,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatorUserSessionInfo {
    pub tweak: Option<Vec<u8>>,
    pub message_hash: Vec<u8>,
    pub metadata: SigningMetadata,
    pub state: AggregatorUserSessionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignerUserKeyState {
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
pub struct SignerUserKeyInfo {
    pub state: SignerUserKeyState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignerUserSessionState {
    SigningRound1 {
        nonces: SigningNonces,
    },
    SigningRound2 {
        signature_share: SignatureShare,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerUserSessionInfo {
    pub tweak: Option<Vec<u8>>,
    pub message_hash: Vec<u8>,
    pub metadata: SigningMetadata,
    pub state: SignerUserSessionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningMetadata {
    pub token_transaction_metadata: TokenTransactionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenTransactionMetadata {
    PartialTokenTransfer {
        token_transaction: TokenTransaction,
    },
    FinalTokenTransfer {
        token_transaction: TokenTransaction,
    },
    PartialCreateToken {
        token_transaction: TokenTransaction,
    },
    FinalCreateToken {
        token_transaction: TokenTransaction,
    },
    PartialMintToken {
        token_transaction: TokenTransaction,
    },
    FinalMintToken {
        token_transaction: TokenTransaction,
    },
}