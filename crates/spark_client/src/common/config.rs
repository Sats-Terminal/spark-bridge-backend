use global_utils::common_types::UrlWrapped;
use serde::{Deserialize, Serialize};
use std::io;
use tonic::transport::Certificate;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SparkOperatorConfig {
    pub base_url: UrlWrapped,
}

#[derive(Debug, Clone)]
pub struct SparkConfig {
    pub operators: Vec<SparkOperatorConfig>,
    pub ca_pem: Certificate,
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
