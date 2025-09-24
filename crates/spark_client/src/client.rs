use crate::utils::time::current_epoch_time_in_seconds;
use crate::{
    common::{config::SparkConfig, error::SparkClientError},
    connection::{SparkServicesClients, SparkTlsConnection},
};
use bitcoin::secp256k1::PublicKey;
use spark_protos::spark::{QueryTokenOutputsRequest, QueryTokenOutputsResponse};
use spark_protos::spark_authn::{
    GetChallengeRequest, GetChallengeResponse, VerifyChallengeRequest, VerifyChallengeResponse,
};
use spark_protos::spark_token::{
    CommitTransactionRequest, CommitTransactionResponse, StartTransactionRequest, StartTransactionResponse,
};
use std::collections::HashMap;
use std::{future::Future, sync::Arc};
use tokio::sync::Mutex;
use tracing;

const N_QUERY_RETRIES: usize = 3;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SparkAuthSession {
    pub session_token: String,
    pub expiration_time: u64,
}

#[derive(Clone)]
pub struct SparkRpcClient {
    clients: SparkServicesClients,
    authn_sessions: Arc<Mutex<HashMap<PublicKey, SparkAuthSession>>>,
}

impl SparkRpcClient {
    pub async fn new(config: SparkConfig) -> Result<Self, SparkClientError> {
        let tls_connection = SparkTlsConnection::new(config)?;
        let clients = tls_connection.create_clients().await?;
        Ok(Self {
            clients,
            authn_sessions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    async fn retry_query<F, Fut, Resp, P>(&self, query_fn: F, params: P) -> Result<Resp, SparkClientError>
    where
        F: Fn(SparkServicesClients, P) -> Fut,
        Fut: Future<Output = Result<Resp, SparkClientError>>,
        P: Clone,
    {
        for _j in 0..N_QUERY_RETRIES {
            let response = query_fn(self.clients.clone(), params.clone()).await;
            match response {
                Ok(response) => {
                    return Ok(response);
                }
                Err(e) => {
                    tracing::error!("Query failed, retry {}/{}: {:?}", _j + 1, N_QUERY_RETRIES, e);
                }
            }
            tracing::info!("Sleeping for 100ms and retrying");
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        Err(SparkClientError::ConnectionError(
            "All retry attempts failed".to_string(),
        ))
    }

    pub async fn get_token_outputs(
        &self,
        request: QueryTokenOutputsRequest,
    ) -> Result<QueryTokenOutputsResponse, SparkClientError> {
        let query_fn = |mut clients: SparkServicesClients, request: QueryTokenOutputsRequest| async move {
            clients
                .spark
                .query_token_outputs(request)
                .await
                .map_err(|e| SparkClientError::ConnectionError(format!("Failed to query balance: {}", e)))
        };

        self.retry_query(query_fn, request).await.map(|r| r.into_inner())
    }

    pub async fn start_token_transaction(
        &self,
        request: StartTransactionRequest,
    ) -> Result<StartTransactionResponse, SparkClientError> {
        let query_fn = |mut clients: SparkServicesClients, request: StartTransactionRequest| async move {
            clients
                .spark_token
                .start_transaction(request)
                .await
                .map_err(|e| SparkClientError::ConnectionError(format!("Failed to start transaction: {}", e)))
        };

        self.retry_query(query_fn, request).await.map(|r| r.into_inner())
    }

    pub async fn commit_token_transaction(
        &self,
        request: CommitTransactionRequest,
    ) -> Result<CommitTransactionResponse, SparkClientError> {
        let query_fn = |mut clients: SparkServicesClients, request: CommitTransactionRequest| async move {
            clients
                .spark_token
                .commit_transaction(request)
                .await
                .map_err(|e| SparkClientError::ConnectionError(format!("Failed to commit transaction: {}", e)))
        };

        self.retry_query(query_fn, request).await.map(|r| r.into_inner())
    }

    pub async fn get_challenge(&self, request: GetChallengeRequest) -> Result<GetChallengeResponse, SparkClientError> {
        let query_fn = |mut clients: SparkServicesClients, request: GetChallengeRequest| async move {
            clients
                .spark_auth
                .get_challenge(request)
                .await
                .map_err(|e| SparkClientError::AuthenticationError(format!("Failed to get challenge: {}", e)))
        };
        self.retry_query(query_fn, request).await.map(|r| r.into_inner())
    }

    pub async fn verify_challenge(
        &self,
        request: VerifyChallengeRequest,
    ) -> Result<VerifyChallengeResponse, SparkClientError> {
        let query_fn = |mut clients: SparkServicesClients, request: VerifyChallengeRequest| async move {
            clients
                .spark_auth
                .verify_challenge(request)
                .await
                .map_err(|e| SparkClientError::AuthenticationError(format!("Failed to verify challenge: {}", e)))
        };
        let public_key = PublicKey::from_slice(&request.public_key)
            .map_err(|e| SparkClientError::DecodeError(format!("Failed to parse public key: {}", e)))?;
        let response = self.retry_query(query_fn, request).await.map(|r| r.into_inner());

        if let Ok(response) = &response {
            let session_token = response.session_token.clone();
            let expiration_time = response.expiration_timestamp;
            let mut authn_sessions = self.authn_sessions.lock().await;
            authn_sessions.insert(
                public_key,
                SparkAuthSession {
                    session_token,
                    expiration_time: expiration_time as u64,
                },
            );
        }

        response
    }

    pub async fn get_auth_session(&self, public_key: PublicKey) -> Option<SparkAuthSession> {
        let mut authn_sessions = self.authn_sessions.lock().await;
        let authn_session = authn_sessions.get(&public_key);

        match authn_session {
            Some(s) => {
                if s.expiration_time < current_epoch_time_in_seconds() {
                    authn_sessions.remove(&public_key);
                    None
                } else {
                    Some(s.clone())
                }
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::config::{CaCertificate, CertificateConfig, SparkConfig, SparkOperatorConfig};
    use global_utils::common_types::{Url, UrlWrapped};
    use global_utils::logger::{LoggerGuard, init_logger};
    use spark_address::decode_spark_address;
    use std::str::FromStr;
    use std::sync::LazyLock;
    use tracing::info;

    const PATH_TO_CA_PEM: &str = "../../infrastructure/configurations/common/ca.pem";

    pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());

    #[tokio::test]
    async fn test_get_balances_direct() -> anyhow::Result<()> {
        let _logger_guard = &*TEST_LOGGER;
        info!("Starting test");

        let address = "sprt1pgss8fxt9jxuv4dgjwrg539s6u06ueausq076xvfej7wdah0htvjlxunt9fa4n".to_string();
        let rune_id = "btknrt1p2sy7a8cx5pqfm3u4p2qfqa475fgwj3eg5d03hhk47t66605zf6qg52vj2".to_string();

        let address_data = decode_spark_address(&address)?;

        let identity_public_key = hex::decode(address_data.identity_public_key)
            .map_err(|e| SparkClientError::DecodeError(format!("Failed to decode identity public key: {}", e)))?;
        let token_identifier = bech32::decode(&rune_id)
            .map_err(|e| SparkClientError::DecodeError(format!("Failed to decode token identifier: {}", e)))?
            .1;

        let request = QueryTokenOutputsRequest {
            owner_public_keys: vec![identity_public_key],
            token_identifiers: vec![token_identifier],
            token_public_keys: vec![],
            network: address_data.network as i32,
        };

        let config = SparkConfig {
            operators: vec![SparkOperatorConfig {
                base_url: UrlWrapped(Url::from_str("https://0.spark.lightspark.com")?),
                id: 0,
                identity_public_key: "".to_string(),
                frost_identifier: "".to_string(),
                running_authority: "".to_string(),
                is_coordinator: Some(true),
            }],
            certificate: CertificateConfig {
                path: PATH_TO_CA_PEM.to_string(),
            },
        };

        let balance_checker = SparkRpcClient::new(config).await.unwrap();

        let response = balance_checker.get_token_outputs(request).await?;

        for output in response.outputs_with_previous_transaction_data {
            if let Some(output) = output.output {
                info!("token identifier: {:?}", hex::encode(output.token_identifier.unwrap()));
                info!("token pubkey: {:?}", hex::encode(output.token_public_key.unwrap()));
                let amount = u128::from_be_bytes(output.token_amount.try_into().unwrap());
                info!("amount: {:?}", amount);
            }
        }
        Ok(())
    }
}
