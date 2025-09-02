use std::{future::Future, sync::Arc};

use hex;
use log;
use spark_protos::spark::{
    QueryTokenOutputsRequest, QueryTokenOutputsResponse, spark_service_client::SparkServiceClient,
};
use tokio::sync::Mutex;
use tonic::transport::Channel;

use crate::{
    common::{config::SparkConfig, error::SparkClientError},
    connection::SparkConnectionPool,
    utils::spark_address::{Network, decode_spark_address},
};

const N_QUERY_RETRIES: usize = 3;
const N_OPERATOR_SWITCHES: usize = 2;

#[derive(Clone)]
pub struct SparkRpcClient {
    connection_pool: Arc<Mutex<SparkConnectionPool>>,
    cached_client: Option<SparkServiceClient<Channel>>,
}

impl SparkRpcClient {
    pub fn new(config: SparkConfig) -> Self {
        let connection_pool = SparkConnectionPool::new(config);
        Self {
            connection_pool: Arc::new(Mutex::new(connection_pool)),
            cached_client: None,
        }
    }

    async fn get_client(&mut self) -> Result<SparkServiceClient<Channel>, SparkClientError> {
        match &self.cached_client {
            Some(client) => Ok(client.clone()),
            None => {
                let mut connection_pool = self.connection_pool.lock().await;
                let client = connection_pool.create_client().await?;
                self.cached_client = Some(client.clone());
                Ok(client)
            }
        }
    }

    async fn switch_operator(&mut self) {
        let mut connection_pool = self.connection_pool.lock().await;
        connection_pool.switch_operator().await;
        self.cached_client = None;
    }

    async fn retry_query<F, Fut, Resp, P>(&mut self, query_fn: F, params: P) -> Result<Resp, SparkClientError>
    where
        F: Fn(SparkServiceClient<Channel>, P) -> Fut,
        Fut: Future<Output = Result<Resp, SparkClientError>>,
        P: Clone,
    {
        for _i in 0..N_OPERATOR_SWITCHES {
            for _j in 0..N_QUERY_RETRIES {
                match self.get_client().await {
                    Ok(client) => {
                        let response = query_fn(client.clone(), params.clone()).await;
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
        spark_address: String,
        token_identifier: String,
    ) -> Result<QueryTokenOutputsResponse, SparkClientError> {
        let address_data = decode_spark_address(spark_address)?;

        let identity_public_key = hex::decode(address_data.identity_public_key)
            .map_err(|e| SparkClientError::DecodeError(format!("Failed to decode identity public key: {}", e)))?;
        let token_identifier = bech32::decode(&token_identifier)
            .map_err(|e| SparkClientError::DecodeError(format!("Failed to decode token identifier: {}", e)))?
            .1;

        let query_fn = |mut client: SparkServiceClient<Channel>, params: (Vec<u8>, Vec<u8>, Network)| async move {
            let request = QueryTokenOutputsRequest {
                owner_public_keys: vec![params.0],
                token_identifiers: vec![params.1],
                token_public_keys: vec![],
                network: params.2 as i32,
            };
            client
                .query_token_outputs(request)
                .await
                .map_err(|e| SparkClientError::ConnectionError(format!("Failed to query balance: {}", e)))
        };

        self.retry_query(query_fn, (identity_public_key, token_identifier, address_data.network))
            .await
            .map(|r| r.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::config::{SparkConfig, SparkOperatorConfig};

    fn init_logger() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .is_test(true)
            .try_init();
    }

    #[tokio::test]
    async fn test_get_balances_direct() {
        init_logger();
        log::info!("Starting test");

        let address = "sprt1pgss8fxt9jxuv4dgjwrg539s6u06ueausq076xvfej7wdah0htvjlxunt9fa4n".to_string();
        let rune_id = "btknrt1p2sy7a8cx5pqfm3u4p2qfqa475fgwj3eg5d03hhk47t66605zf6qg52vj2".to_string();

        let config = SparkConfig {
            operators: vec![SparkOperatorConfig {
                base_url: "https://0.spark.lightspark.com".to_string(),
            }],
            ca_pem_path: "../../ca.pem".to_string(),
        };

        let mut balance_checker = SparkRpcClient::new(config);

        let response = balance_checker.get_token_outputs(address, rune_id).await.unwrap();

        for output in response.outputs_with_previous_transaction_data {
            if let Some(output) = output.output {
                log::info!("token identifier: {:?}", hex::encode(output.token_identifier.unwrap()));
                log::info!("token pubkey: {:?}", hex::encode(output.token_public_key.unwrap()));
                let amount = u128::from_be_bytes(output.token_amount.try_into().unwrap());
                log::info!("amount: {:?}", amount);
            }
        }
    }
}
