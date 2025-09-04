use spark_client::client::SparkRpcClient;
use frost::aggregator::FrostAggregator;
use crate::errors::SparkServiceError;
use crate::types::SparkTransactionRequest;
use lrc20::token_transaction::TokenTransaction;
use bitcoin::hashes::Hash;
use crate::types::create_partial_token_transaction;

pub struct SparkService {
    spark_client: SparkRpcClient,
    frost_aggregator: FrostAggregator,
}

impl SparkService {
    pub fn new(
        spark_client: SparkRpcClient,
        frost_aggregator: FrostAggregator,
    ) -> Self {
        Self { spark_client, frost_aggregator }
    }

    pub async fn send_spark_transaction(
        &self,
        transaction: SparkTransactionRequest,
    ) -> Result<(), SparkServiceError> {
        let token_transaction = create_partial_token_transaction(transaction)?;

        let partial_token_transaction_hash = token_transaction.hash()
            .map_err(|err| SparkServiceError::HashError(err.to_string()))?;

        let partial_token_transaction_hash_bytes = partial_token_transaction_hash.to_byte_array();

        Ok(())
    }
}