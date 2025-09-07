use spark_client::client::SparkRpcClient;
use frost::aggregator::FrostAggregator;
use frost::types::MusigId;
use lrc20::token_identifier::TokenIdentifier;
use spark_client::utils::spark_address::Network;
use bitcoin::secp256k1::PublicKey;
use crate::errors::SparkServiceError;
use crate::types::*;
use frost::types::SigningMetadata;
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
        issuer_id: MusigId,
        token_identifier: TokenIdentifier,
        transaction_type: SparkTransactionType,
        network: Network,
        spark_operator_identity_public_keys: Vec<PublicKey>,
    ) -> Result<(), SparkServiceError> {
        let public_key_package = self.frost_aggregator.get_public_key_package(issuer_id.clone(), None).await
            .map_err(|err| SparkServiceError::FrostAggregatorError(err.to_string()))?;

        let issuer_public_key = PublicKey::from_slice(
            &public_key_package
                .verifying_key()
                .serialize()
                .map_err(|err| SparkServiceError::InvalidData(format!("Failed to serialize public key: {:?}", err)))?
                .as_slice()
        ).map_err(|err| SparkServiceError::InvalidData(format!("Failed to parse public key: {:?}", err)))?;

        let token_transaction = create_partial_token_transaction(
            issuer_public_key, 
            transaction_type.clone(), 
            token_identifier, 
            spark_operator_identity_public_keys, 
            network
        )?;

        let partial_token_transaction_hash = token_transaction.hash()
            .map_err(|err| SparkServiceError::HashError(err.to_string()))?;

        let partial_token_transaction_hash_bytes = partial_token_transaction_hash.to_byte_array();

        let signing_metadata = create_signing_metadata(
            token_transaction,
            transaction_type.clone(),
            true
        );

        let signature = self.frost_aggregator
            .run_signing_flow(
                issuer_id.clone(), 
                partial_token_transaction_hash_bytes.as_slice(), 
                signing_metadata,
                None
            ).await
            .map_err(|e| SparkServiceError::FrostAggregatorError(e.to_string()))?;

        

        Ok(())
    }
}