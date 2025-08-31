use frost_secp256k1::Identifier;
use serde::{Deserialize, Serialize};

use crate::errors::{Result, SignerError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerConfig {
    pub participant_id: Identifier,
    pub threshold: u32,
    pub total_participants: u32,
    pub private_key_hex: Option<String>,
}

impl SignerConfig {
    pub fn new(participant_id: u16, threshold: u32, total_participants: u32) -> Result<Self> {
        if threshold == 0 || threshold > total_participants {
            return Err(SignerError::ThresholdTooHigh {
                threshold,
                participants: total_participants,
            });
        }

        let participant_id = Identifier::try_from(participant_id)
            .map_err(|_| SignerError::ConfigError {
                reason: "Invalid participant_id".to_string(),
            })?;

        Ok(Self {
            participant_id,
            threshold,
            total_participants,
            private_key_hex: None,
        })
    }

    pub fn with_private_key(mut self, private_key_hex: String) -> Self {
        self.private_key_hex = Some(private_key_hex);
        self
    }

    pub fn from_env() -> Result<Self> { // magic string - I will change it
        let participant_id: u16 = std::env::var("PARTICIPANT_ID")
            .map_err(|_| SignerError::ConfigError {
                reason: "PARTICIPANT_ID not set".to_string(),
            })?
            .parse()
            .map_err(|_| SignerError::ConfigError {
                reason: "Invalid PARTICIPANT_ID value".to_string(),
            })?;

        let threshold: u32 = std::env::var("THRESHOLD")
            .map_err(|_| SignerError::ConfigError {
                reason: "THRESHOLD not set".to_string(),
            })?
            .parse()
            .map_err(|_| SignerError::ConfigError {
                reason: "Invalid THRESHOLD value".to_string(),
            })?;

        let total_participants: u32 = std::env::var("TOTAL_PARTICIPANTS")
            .map_err(|_| SignerError::ConfigError {
                reason: "TOTAL_PARTICIPANTS not set".to_string(),
            })?
            .parse()
            .map_err(|_| SignerError::ConfigError {
                reason: "Invalid TOTAL_PARTICIPANTS value".to_string(),
            })?;

        let private_key_hex = std::env::var("PRIVATE_KEY_HEX").ok();

        let mut config = Self::new(participant_id, threshold, total_participants)?;
        if let Some(pk) = private_key_hex {
            config = config.with_private_key(pk);
        }

        Ok(config)
    }
}
