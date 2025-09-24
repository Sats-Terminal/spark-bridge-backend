use crate::common::error::SparkClientError;
use global_utils::common_types::UrlWrapped;
use serde::{Deserialize, Serialize};
use std::io;
use tonic::transport::Certificate;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SparkOperatorConfig {
    pub id: u32,
    pub base_url: UrlWrapped,
    pub identity_public_key: String,
    pub frost_identifier: String,
    pub running_authority: String,
    pub is_coordinator: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateConfig {
    pub path: String,
}

impl CertificateConfig {
    pub fn get_certificate(&self) -> Result<Certificate, SparkClientError> {
        let file = std::fs::read(self.path.clone())
            .map_err(|e| SparkClientError::ConfigError(format!("Failed to read certificate: {}", e)))?;
        Ok(Certificate::from_pem(file))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparkConfig {
    pub operators: Vec<SparkOperatorConfig>,
    pub certificates: Vec<CertificateConfig>,
}

impl SparkConfig {
    pub fn coordinator_operator(&self) -> Result<usize, SparkClientError> {
        for i in 0..self.operators.len() {
            if self.operators[i].is_coordinator.unwrap_or(false) {
                return Ok(i);
            }
        }
        Err(SparkClientError::ConfigError(
            "Coordinator operator not found".to_string(),
        ))
    }
}

#[derive(Debug)]
pub struct CaCertificate {
    pub ca_pem: Certificate,
}

impl CaCertificate {
    pub fn from_path(path: impl AsRef<str>) -> Result<Self, io::Error> {
        let file = std::fs::read(path.as_ref())?;
        Ok(CaCertificate {
            ca_pem: Certificate::from_pem(file),
        })
    }
}
