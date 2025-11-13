use global_utils::http_client::{HttpClient, HttpClientError};
use reqwest::header::{AUTHORIZATION, HeaderMap};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::models::{Network, TxV1Response};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SparkScanConfig {
    pub api_key: String,
    pub base_url: Url,
}

#[derive(Clone)]
pub struct SparkScanClient {
    api_key: String,
    client: HttpClient,
}

impl SparkScanClient {
    pub fn new(config: SparkScanConfig) -> Self {
        SparkScanClient {
            api_key: config.api_key,
            client: HttpClient::new(config.base_url),
        }
    }
}

impl SparkScanClient {
    fn headers(&self) -> Result<HeaderMap, HttpClientError> {
        let mut headers = HeaderMap::with_capacity(1);
        headers.insert(AUTHORIZATION, format!("Bearer {}", self.api_key).parse()?);
        Ok(headers)
    }

    pub async fn get_transaction(&self, txid: &str, network: Network) -> Result<TxV1Response, HttpClientError> {
        self.client
            .get(
                &format!("v1/tx/{}", txid),
                Some(&[("network", network)]),
                Some(self.headers()?),
            )
            .await
    }
}
