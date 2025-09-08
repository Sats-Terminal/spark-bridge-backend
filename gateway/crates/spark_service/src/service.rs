use spark_client::client::SparkRpcClient;
use frost::aggregator::FrostAggregator;
use frost::types::MusigId;
use lrc20::token_identifier::TokenIdentifier;
use spark_client::utils::spark_address::Network;
use bitcoin::secp256k1::PublicKey;
use crate::errors::SparkServiceError;
use crate::types::*;
use crate::types::create_partial_token_transaction;
use lrc20::proto_hasher::hash_token_transaction;
use lrc20::proto_hasher::get_descriptor_pool;
use prost_reflect::DescriptorPool;

pub struct SparkService {
    spark_client: SparkRpcClient,
    frost_aggregator: FrostAggregator,
    descriptor_pool: DescriptorPool,
}

impl SparkService {
    pub fn new(
        spark_client: SparkRpcClient,
        frost_aggregator: FrostAggregator,
    ) -> Self {
        Self { 
            spark_client, 
            frost_aggregator, 
            descriptor_pool: get_descriptor_pool() 
        }
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

        let partial_token_transaction_hash = hash_token_transaction(self.descriptor_pool.clone(), token_transaction, true)
            .map_err(|err| SparkServiceError::HashError(format!("Failed to hash partial token transaction: {:?}", err)))?;

        

        Ok(())
    }
}