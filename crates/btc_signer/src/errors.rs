use thiserror::Error;

#[derive(Error, Debug)]
pub enum SignerError {
    #[error("Cryptographic error: {0}")]
    CryptoError(#[from] secp256k1::Error),

    #[error("Invalid key share")]
    InvalidKeyShare,

    #[error("Invalid nonce commitment")]
    InvalidNonceCommitment,

    #[error("Invalid partial signature")]
    InvalidPartialSignature,

    #[error("DKG round {round} failed: {reason}")]
    DkgRoundFailed { round: u8, reason: String },

    #[error("Signing round {round} failed: {reason}")]
    SigningRoundFailed { round: u8, reason: String },

    #[error("Invalid participant count: {count}")]
    InvalidParticipantCount { count: usize },

    #[error("Threshold too high: {threshold} > {participants}")]
    ThresholdTooHigh { threshold: u32, participants: u32 },

    #[error("Insufficient participants: got {got}, need {need}")]
    InsufficientParticipants { got: usize, need: usize },

    #[error("Participant {id} not found")]
    ParticipantNotFound { id: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),

    #[error("Hex decode error: {0}")]
    HexError(#[from] hex::FromHexError),

    #[error("Invalid configuration: {reason}")]
    ConfigError { reason: String },

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, SignerError>;