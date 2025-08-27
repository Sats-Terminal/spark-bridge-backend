use thiserror::Error;

#[derive(Error, Debug)]
pub enum AggregatorError {
    #[error("Signer error: {0}")]
    SignerError(#[from] btc_signer::SignerError),

    #[error("Cryptographic error: {0}")]
    CryptoError(#[from] secp256k1::Error),

    #[error("Session {id} not found")]
    SessionNotFound { id: String },

    #[error("Invalid session state: {state}")]
    InvalidSessionState { state: String },

    #[error("Insufficient participants: got {got}, need {need}")]
    InsufficientParticipants { got: usize, need: usize },

    #[error("Participant {id} already exists")]
    ParticipantExists { id: String },

    #[error("Participant {id} not found")]
    ParticipantNotFound { id: String },

    #[error("DKG failed at round {round}: {reason}")]
    DkgFailed { round: u8, reason: String },

    #[error("Signing failed: {reason}")]
    SigningFailed { reason: String },

    #[error("Invalid signature aggregation")]
    InvalidAggregation,

    #[error("Session timeout")]
    SessionTimeout,

    #[error("Configuration error: {reason}")]
    ConfigError { reason: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, AggregatorError>;