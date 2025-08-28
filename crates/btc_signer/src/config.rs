use serde::{Deserialize, Serialize};
use btc_signer_types::types::ParticipantId;

use crate::errors::Result;
use crate::errors::SignerError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerConfig {
    pub participant_id: ParticipantId,
    pub threshold: u32,
    pub total_participants: u32,
    pub private_key_hex: Option<String>,
}

impl SignerConfig {
    pub fn new(participant_id: String, threshold: u32, total_participants: u32) -> Result<Self> {
        if threshold == 0 || threshold > total_participants {
            return Err(SignerError::ThresholdTooHigh {
                threshold,
                participants: total_participants,
            });
        }

        Ok(Self {
            participant_id: participant_id.into(),
            threshold,
            total_participants,
            private_key_hex: None,
        })
    }

    pub fn with_private_key(mut self, private_key_hex: String) -> Self {
        self.private_key_hex = Some(private_key_hex);
        self
    }

    pub fn from_env() -> Result<Self> {
        let participant_id = std::env::var("PARTICIPANT_ID")
            .map_err(|_| SignerError::ConfigError {
                reason: "PARTICIPANT_ID not set".to_string(),
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