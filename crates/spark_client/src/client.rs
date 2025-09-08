use crate::{
    common::{config::SparkConfig, error::SparkClientError},
    connection::{SparkClients, SparkConnectionPool},
};
use log;
use spark_protos::spark::{QueryTokenOutputsRequest, QueryTokenOutputsResponse};
use spark_protos::spark_token::{
    CommitTransactionRequest, CommitTransactionResponse, StartTransactionRequest, StartTransactionResponse,
};
use std::{future::Future, sync::Arc};
use tokio::sync::Mutex;

const N_QUERY_RETRIES: usize = 3;
const N_OPERATOR_SWITCHES: usize = 2;
const DEFAULT_VALIDITY_DURATION_SECONDS: u64 = 300;

#[derive(Clone)]
pub struct SparkRpcClient {
    connection_pool: Arc<Mutex<SparkConnectionPool>>,
    cached_clients: Option<SparkClients>,
}

impl SparkRpcClient {
    pub fn new(config: SparkConfig) -> Self {
        let connection_pool = SparkConnectionPool::new(config);
        Self {
            connection_pool: Arc::new(Mutex::new(connection_pool)),
            cached_clients: None,
        }
    }

    async fn get_clients(&mut self) -> Result<SparkClients, SparkClientError> {
        match &self.cached_clients {
            Some(client) => Ok(client.clone()),
            None => {
                let mut connection_pool = self.connection_pool.lock().await;
                let client = connection_pool.create_clients().await?;
                self.cached_clients = Some(client.clone());
                Ok(client)
            }
        }
    }

    async fn switch_operator(&mut self) {
        let mut connection_pool = self.connection_pool.lock().await;
        connection_pool.switch_operator().await;
        self.cached_clients = None;
    }

    async fn retry_query<F, Fut, Resp, P>(&mut self, query_fn: F, params: P) -> Result<Resp, SparkClientError>
    where
        F: Fn(SparkClients, P) -> Fut,
        Fut: Future<Output = Result<Resp, SparkClientError>>,
        P: Clone,
    {
        for _i in 0..N_OPERATOR_SWITCHES {
            for _j in 0..N_QUERY_RETRIES {
                match self.get_clients().await {
                    Ok(clients) => {
                        let response = query_fn(clients.clone(), params.clone()).await;
                        match response {
                            Ok(response) => {
                                return Ok(response);
                            }
                            Err(e) => {
                                log::error!("Query failed, retry {}/{}: {:?}", _j + 1, N_QUERY_RETRIES, e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to create client: {:?}", e);
                    }
                }
                log::info!("Sleeping for 100ms and retrying");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            self.switch_operator().await;
            log::info!("Switching operator");
        }

        Err(SparkClientError::ConnectionError(
            "All retry attempts failed".to_string(),
        ))
    }

    pub async fn get_token_outputs(
        &mut self,
        request: QueryTokenOutputsRequest,
    ) -> Result<QueryTokenOutputsResponse, SparkClientError> {
        let query_fn = |mut clients: SparkClients, request: QueryTokenOutputsRequest| async move {
            clients
                .spark
                .query_token_outputs(request)
                .await
                .map_err(|e| SparkClientError::ConnectionError(format!("Failed to query balance: {}", e)))
        };

        self.retry_query(query_fn, request).await.map(|r| r.into_inner())
    }

    pub async fn start_token_transaction(
        &mut self,
        request: StartTransactionRequest,
    ) -> Result<StartTransactionResponse, SparkClientError> {
        let query_fn = |mut clients: SparkClients, request: StartTransactionRequest| async move {
            clients
                .spark_token
                .start_transaction(request)
                .await
                .map_err(|e| SparkClientError::ConnectionError(format!("Failed to start transaction: {}", e)))
        };

        self.retry_query(query_fn, request).await.map(|r| r.into_inner())
    }

    pub async fn commit_token_transaction(
        &mut self,
        request: CommitTransactionRequest,
    ) -> Result<CommitTransactionResponse, SparkClientError> {
        let query_fn = |mut clients: SparkClients, request: CommitTransactionRequest| async move {
            clients
                .spark_token
                .commit_transaction(request)
                .await
                .map_err(|e| SparkClientError::ConnectionError(format!("Failed to commit transaction: {}", e)))
        };

        self.retry_query(query_fn, request).await.map(|r| r.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::config::{CaCertificate, SparkConfig, SparkOperatorConfig};
    use global_utils::common_types::{Url, UrlWrapped};
    use std::str::FromStr;

    fn init_logger() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .is_test(true)
            .try_init();
    }

    #[tokio::test]
    async fn test_get_balances_direct() -> anyhow::Result<()> {
        init_logger();
        log::info!("Starting test");

        let address = "sprt1pgss8fxt9jxuv4dgjwrg539s6u06ueausq076xvfej7wdah0htvjlxunt9fa4n".to_string();
        let rune_id = "btknrt1p2sy7a8cx5pqfm3u4p2qfqa475fgwj3eg5d03hhk47t66605zf6qg52vj2".to_string();

        let address_data = decode_spark_address(address)?;

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
            }],
            ca_pem: CaCertificate::from_path("../../spark_balance_checker/infrastructure/configuration/ca.pem")?.ca_pem,
        };

        let mut balance_checker = SparkRpcClient::new(config);

        let response = balance_checker.get_token_outputs(request).await?;

        for output in response.outputs_with_previous_transaction_data {
            if let Some(output) = output.output {
                log::info!("token identifier: {:?}", hex::encode(output.token_identifier.unwrap()));
                log::info!("token pubkey: {:?}", hex::encode(output.token_public_key.unwrap()));
                let amount = u128::from_be_bytes(output.token_amount.try_into().unwrap());
                log::info!("amount: {:?}", amount);
            }
        }
        Ok(())
    }
}
