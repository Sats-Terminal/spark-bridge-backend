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
use spark_protos::spark_token::StartTransactionRequest;
use spark_protos::spark_token::SignatureWithIndex;
use spark_protos::spark_token::CommitTransactionRequest;
use lrc20::marshal::marshal_token_transaction;
use lrc20::marshal::unmarshal_token_transaction;
use bitcoin::hashes::sha256::Hash as Sha256Hash;
use bitcoin::hashes::{Hash, HashEngine};
use spark_protos::spark_token::InputTtxoSignaturesPerOperator;
use spark_protos::spark_authn::GetChallengeRequest;
use spark_protos::spark_authn::VerifyChallengeRequest;
use frost::types::SigningMetadata;
use frost::types::TokenTransactionMetadata;
use futures::future::join_all;

const DEFAULT_VALIDITY_DURATION_SECONDS: u64 = 300;

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

    async fn get_musig_public_key(
        &self,
        issuer_id: MusigId,
        nonce_tweak: Option<&[u8]>,
    ) -> Result<PublicKey, SparkServiceError> {
        let public_key_package = self.frost_aggregator.get_public_key_package(issuer_id.clone(), nonce_tweak).await
            .map_err(|err| SparkServiceError::FrostAggregatorError(err.to_string()))?;

        let issuer_public_key_bytes = public_key_package.verifying_key().serialize()
            .map_err(|err| SparkServiceError::InvalidData(format!("Failed to serialize public key: {:?}", err)))?;

        let issuer_public_key = PublicKey::from_slice(&issuer_public_key_bytes)
            .map_err(|err| SparkServiceError::DecodeError(format!("Failed to parse public key: {:?}", err)))?;

        Ok(issuer_public_key)
    }

    async fn authenticate(
        &self,
        musig_id: MusigId,
        nonce_tweak: Option<&[u8]>,
    ) -> Result<(), SparkServiceError> {
        let identity_public_key = self.get_musig_public_key(musig_id.clone(), nonce_tweak).await?;

        let session_token = self.spark_client.get_auth_session(identity_public_key).await;
        if let Some(_) = session_token {
            return Ok(());
        }

        let response = self.spark_client.get_challenge(GetChallengeRequest {
            public_key: identity_public_key.serialize().to_vec(),
        }).await.map_err(|e| SparkServiceError::SparkClientError(format!("Failed to get challenge: {}", e)))?;

        let protected_challenge = response.protected_challenge;
        let challenge = protected_challenge.clone()
            .ok_or(SparkServiceError::DecodeError("Challenge is not found".to_string()))?
            .challenge
            .ok_or(SparkServiceError::DecodeError("Challenge is not found".to_string()))?;
        
        let signature = self.frost_aggregator.run_signing_flow(
            musig_id.clone(), 
            challenge.nonce.as_ref(), 
            SigningMetadata {
                token_transaction_metadata: TokenTransactionMetadata::Authorization,
            },
            nonce_tweak
        ).await
            .map_err(|e| SparkServiceError::FrostAggregatorError(e.to_string()))?;

        let signature_bytes = signature.serialize().map_err(|e| SparkServiceError::DecodeError(format!("Failed to serialize signature: {:?}", e)))?;

        let _ = self.spark_client.verify_challenge(VerifyChallengeRequest {
            protected_challenge,
            signature: signature_bytes.to_vec(),
            public_key: identity_public_key.serialize().to_vec(),
        }).await.map_err(|e| SparkServiceError::SparkClientError(format!("Failed to verify challenge: {}", e)))?;

        Ok(())
    }

    pub async fn send_spark_transaction(
        &self,
        musig_id: MusigId,
        nonce_tweak: Option<&[u8]>,
        token_identifier: TokenIdentifier,
        transaction_type: SparkTransactionType,
        network: Network,
        spark_operator_identity_public_keys: Vec<PublicKey>,
    ) -> Result<(), SparkServiceError> {
        self.authenticate(musig_id.clone(), nonce_tweak).await?;
        
        let identity_public_key = self.get_musig_public_key(musig_id.clone(), nonce_tweak).await?;

        // ----- Start the transaction -----

        let partial_token_transaction = create_partial_token_transaction(
            identity_public_key, 
            transaction_type.clone(), 
            token_identifier, 
            spark_operator_identity_public_keys.clone(), 
            network
        )?;

        let partial_token_transaction_proto = marshal_token_transaction(partial_token_transaction.clone(), false)
            .map_err(|e| SparkServiceError::InvalidData(format!("Failed to marshal partial token transaction: {:?}", e)))?;

        let partial_token_transaction_hash = hash_token_transaction(self.descriptor_pool.clone(), partial_token_transaction_proto.clone())
            .map_err(|err| SparkServiceError::HashError(format!("Failed to hash partial token transaction: {:?}", err)))?;

        let signature = self.frost_aggregator.run_signing_flow(
            musig_id.clone(), 
            partial_token_transaction_hash.as_ref(), 
            create_signing_metadata(partial_token_transaction.clone(), transaction_type.clone(), true),
            nonce_tweak
        ).await
            .map_err(|e| SparkServiceError::FrostAggregatorError(e.to_string()))?;

        let response = self.spark_client.start_token_transaction(StartTransactionRequest {
            identity_public_key: identity_public_key.serialize().to_vec(),
            partial_token_transaction: Some(partial_token_transaction_proto),
            partial_token_transaction_owner_signatures: vec![SignatureWithIndex {
                signature: signature.serialize().map_err(|e| SparkServiceError::DecodeError(format!("Failed to serialize signature: {:?}", e)))?.to_vec(),
                input_index: 0,
            }],
            validity_duration_seconds: DEFAULT_VALIDITY_DURATION_SECONDS,
        }).await.map_err(|e| SparkServiceError::SparkClientError(e.to_string()))?;

        // ----- Finalize the transaction -----

        let final_token_transaction_proto = response.final_token_transaction
            .ok_or(SparkServiceError::DecodeError("Final token transaction is not found".to_string()))?;

        let final_token_transaction = unmarshal_token_transaction(final_token_transaction_proto.clone())
            .map_err(|e| SparkServiceError::DecodeError(format!("Failed to unmarshal final token transaction: {:?}", e)))?;

        let final_token_transaction_hash = hash_token_transaction(self.descriptor_pool.clone(), final_token_transaction_proto.clone())
            .map_err(|err| SparkServiceError::HashError(format!("Failed to hash final token transaction: {:?}", err)))?;

        let mut join_handles = vec![];

        for operator_public_key in spark_operator_identity_public_keys {
            let operator_specific_signable_payload = hash_operator_specific_signable_payload(final_token_transaction_hash, operator_public_key)
                .map_err(|err| SparkServiceError::HashError(format!("Failed to hash operator specific signable payload: {:?}", err)))?;

            let musig_id = musig_id.clone();
            let final_token_transaction = final_token_transaction.clone();
            let transaction_type = transaction_type.clone();

            let join_handle = async move {
                let signature = self.frost_aggregator.run_signing_flow(
                    musig_id, 
                    operator_specific_signable_payload.as_ref(), 
                    create_signing_metadata(final_token_transaction, transaction_type, false),
                    nonce_tweak
                ).await
                    .map_err(|e| SparkServiceError::FrostAggregatorError(e.to_string()))?;

                let ttxo_signatures = InputTtxoSignaturesPerOperator {
                    ttxo_signatures: vec![SignatureWithIndex {
                        signature: signature.serialize().map_err(|e| SparkServiceError::DecodeError(format!("Failed to serialize signature: {:?}", e)))?.to_vec(),
                        input_index: 0,
                    }],
                    operator_identity_public_key: operator_public_key.serialize().to_vec(),
                };

                Ok(ttxo_signatures)
            };
            join_handles.push(join_handle);
        }

        let signatures = join_all(join_handles)
            .await
            .into_iter()
            .collect::<Result<Vec<InputTtxoSignaturesPerOperator>, SparkServiceError>>()?;

        let _ = self.spark_client.commit_token_transaction(CommitTransactionRequest {
            final_token_transaction: Some(final_token_transaction_proto),
            final_token_transaction_hash: final_token_transaction_hash.to_byte_array().to_vec(),
            input_ttxo_signatures_per_operator: signatures,
            owner_identity_public_key: identity_public_key.serialize().to_vec(),
        }).await.map_err(|e| SparkServiceError::SparkClientError(e.to_string()))?;

        Ok(())
    }
}

fn hash_operator_specific_signable_payload(
    token_tx_hash: Sha256Hash,
    operator_public_key: PublicKey, // this must always be 33 bytes
) -> Result<Sha256Hash, Box<dyn std::error::Error>> {

    let mut engine = Sha256Hash::engine();
    engine.input(Sha256Hash::hash(token_tx_hash.as_byte_array().as_slice()).as_byte_array());
    engine.input(Sha256Hash::hash(operator_public_key.serialize().as_slice()).as_byte_array());
    let final_hash = Sha256Hash::from_engine(engine);

    Ok(final_hash)
}
