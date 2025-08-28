use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::errors::AggregatorError;
use crate::errors::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatorConfig {
    pub threshold: u32,
    pub total_participants: u32,
    pub session_timeout: Duration,
    pub max_concurrent_sessions: usize,
    pub cleanup_interval: Duration,
}

impl Default for AggregatorConfig {
    fn default() -> Self {
        Self {
            threshold: 2,
            total_participants: 3,
            session_timeout: Duration::from_secs(3600), 
            max_concurrent_sessions: 100,
            cleanup_interval: Duration::from_secs(300),
        }
    }
}

impl AggregatorConfig {
    pub fn new(threshold: u32, total_participants: u32) -> Result<Self> {
        if threshold == 0 {
            return Err(AggregatorError::ConfigError {
                reason: "Threshold cannot be zero".to_string(),
            });
        }

        if threshold > total_participants {
            return Err(AggregatorError::ConfigError {
                reason: format!("Threshold {} cannot exceed total participants {}",
                                threshold, total_participants),
            });
        }

        Ok(Self {
            threshold,
            total_participants,
            ..Default::default()
        })
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.session_timeout = timeout;
        self
    }

    pub fn with_max_sessions(mut self, max_sessions: usize) -> Self {
        self.max_concurrent_sessions = max_sessions;
        self
    }

    pub fn from_env() -> Result<Self> {
        let threshold = std::env::var("AGGREGATOR_THRESHOLD")
            .unwrap_or_else(|_| "2".to_string())
            .parse()
            .map_err(|_| AggregatorError::ConfigError {
                reason: "Invalid AGGREGATOR_THRESHOLD value".to_string(),
            })?;

        let total_participants = std::env::var("AGGREGATOR_TOTAL_PARTICIPANTS")
            .unwrap_or_else(|_| "3".to_string())
            .parse()
            .map_err(|_| AggregatorError::ConfigError {
                reason: "Invalid AGGREGATOR_TOTAL_PARTICIPANTS value".to_string(),
            })?;

        let timeout_secs = std::env::var("AGGREGATOR_TIMEOUT")
            .unwrap_or_else(|_| "3600".to_string())
            .parse()
            .map_err(|_| AggregatorError::ConfigError {
                reason: "Invalid AGGREGATOR_TIMEOUT value".to_string(),
            })?;

        let max_sessions = std::env::var("AGGREGATOR_MAX_SESSIONS")
            .unwrap_or_else(|_| "100".to_string())
            .parse()
            .map_err(|_| AggregatorError::ConfigError {
                reason: "Invalid AGGREGATOR_MAX_SESSIONS value".to_string(),
            })?;

        Ok(Self {
            threshold,
            total_participants,
            session_timeout: Duration::from_secs(timeout_secs),
            max_concurrent_sessions: max_sessions,
            ..Default::default()
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.threshold == 0 {
            return Err(AggregatorError::ConfigError {
                reason: "Threshold cannot be zero".to_string(),
            });
        }

        if self.threshold > self.total_participants {
            return Err(AggregatorError::ConfigError {
                reason: format!("Threshold {} exceeds total participants {}",
                                self.threshold, self.total_participants),
            });
        }

        if self.max_concurrent_sessions == 0 {
            return Err(AggregatorError::ConfigError {
                reason: "Max concurrent sessions cannot be zero".to_string(),
            });
        }

        Ok(())
    }
}