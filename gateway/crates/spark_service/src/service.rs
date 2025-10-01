use crate::errors::SparkServiceError;
use crate::types::create_partial_token_transaction;
use crate::types::*;
use crate::utils::spark_network_to_proto_network;
use bitcoin::hashes::sha256::Hash as Sha256Hash;
use bitcoin::hashes::{Hash, HashEngine, sha256};
use bitcoin::secp256k1::PublicKey;
use frost::aggregator::FrostAggregator;
use frost::types::MusigId;
use frost::types::SigningMetadata;
use frost::types::TweakBytes;
use futures::future::join_all;
use lrc20::marshal::marshal_token_transaction;
use lrc20::marshal::unmarshal_token_transaction;
use proto_hasher::ProtoHasher;
use spark_address::Network;
use spark_client::client::SparkRpcClient;
use spark_protos::prost::Message;
use spark_protos::reflect::ToDynamicMessage;
use spark_protos::spark_authn::GetChallengeRequest;
use spark_protos::spark_authn::VerifyChallengeRequest;
use spark_protos::spark_token::CommitTransactionRequest;
use spark_protos::spark_token::InputTtxoSignaturesPerOperator;
use spark_protos::spark_token::SignatureWithIndex;
use spark_protos::spark_token::StartTransactionRequest;
use token_identifier::TokenIdentifier;

const DEFAULT_VALIDITY_DURATION_SECONDS: u64 = 300;

pub struct SparkService {
    spark_client: SparkRpcClient,
    frost_aggregator: FrostAggregator,
    proto_hasher: ProtoHasher,
    spark_operator_identity_public_keys: Vec<PublicKey>,
}

impl SparkService {
    pub fn new(
        spark_client: SparkRpcClient,
        frost_aggregator: FrostAggregator,
        spark_operator_identity_public_keys: Vec<PublicKey>,
    ) -> Self {
        Self {
            spark_client,
            frost_aggregator,
            proto_hasher: ProtoHasher::new(),
            spark_operator_identity_public_keys,
        }
    }

    async fn get_musig_public_key(
        &self,
        issuer_id: MusigId,
        nonce_tweak: Option<TweakBytes>,
    ) -> Result<PublicKey, SparkServiceError> {
        let public_key_package = self
            .frost_aggregator
            .get_public_key_package(issuer_id.clone(), nonce_tweak)
            .await
            .map_err(|err| SparkServiceError::FrostAggregatorError(err.to_string()))?;

        let issuer_public_key_bytes = public_key_package
            .verifying_key()
            .serialize()
            .map_err(|err| SparkServiceError::InvalidData(format!("Failed to serialize public key: {:?}", err)))?;

        let issuer_public_key = PublicKey::from_slice(&issuer_public_key_bytes)
            .map_err(|err| SparkServiceError::DecodeError(format!("Failed to parse public key: {:?}", err)))?;

        Ok(issuer_public_key)
    }

    async fn authenticate(&self, musig_id: MusigId, nonce_tweak: Option<TweakBytes>) -> Result<(), SparkServiceError> {
        tracing::debug!(
            "Authenticating with musig id: {:?}, nonce tweak: {:?}",
            musig_id,
            nonce_tweak
        );
        let identity_public_key = self.get_musig_public_key(musig_id.clone(), nonce_tweak).await?;

        let session_token = self.spark_client.get_auth_session(identity_public_key).await;
        if let Some(_) = session_token {
            return Ok(());
        }

        let response = self
            .spark_client
            .get_challenge(GetChallengeRequest {
                public_key: identity_public_key.serialize().to_vec(),
            })
            .await
            .map_err(|e| SparkServiceError::SparkClientError(format!("Failed to get challenge: {}", e)))?;

        let protected_challenge = response.protected_challenge;
        let challenge = protected_challenge
            .clone()
            .ok_or(SparkServiceError::DecodeError("Challenge is not found".to_string()))?
            .challenge
            .ok_or(SparkServiceError::DecodeError("Challenge is not found".to_string()))?;

        let message_hash = sha256::Hash::hash(challenge.encode_to_vec().as_slice());

        let signature = self
            .frost_aggregator
            .run_signing_flow(
                musig_id.clone(),
                message_hash.as_byte_array().as_slice(),
                SigningMetadata::Authorization,
                nonce_tweak,
            )
            .await
            .map_err(|e| SparkServiceError::FrostAggregatorError(e.to_string()))?;

        let signature_bytes = signature
            .serialize()
            .map_err(|e| SparkServiceError::DecodeError(format!("Failed to serialize signature: {:?}", e)))?;

        let _ = self
            .spark_client
            .verify_challenge(VerifyChallengeRequest {
                protected_challenge,
                signature: signature_bytes.to_vec(),
                public_key: identity_public_key.serialize().to_vec(),
            })
            .await
            .map_err(|e| SparkServiceError::SparkClientError(format!("Failed to verify challenge: {}", e)))?;

        tracing::debug!("Challenge verified for musig id: {:?}", musig_id);

        Ok(())
    }

    pub async fn send_spark_transaction(
        &self,
        musig_id: MusigId,
        nonce_tweak: Option<TweakBytes>,
        token_identifier: TokenIdentifier,
        transaction_type: SparkTransactionType,
        network: Network,
    ) -> Result<(), SparkServiceError> {
        tracing::debug!(
            "Send spark transaction with musig id: {:?}, nonce tweak: {:?}, token identifier: {:?}",
            musig_id,
            nonce_tweak,
            token_identifier.to_string()
        );

        self.authenticate(musig_id.clone(), nonce_tweak).await?;

        let identity_public_key = self.get_musig_public_key(musig_id.clone(), nonce_tweak).await?;

        tracing::debug!("Transaction identity public key: {:?}", identity_public_key.to_string());

        // ----- Start the transaction -----

        tracing::debug!("Start the transaction");

        let partial_token_transaction = create_partial_token_transaction(
            identity_public_key,
            transaction_type.clone(),
            token_identifier,
            self.spark_operator_identity_public_keys.clone(),
            spark_network_to_proto_network(network),
        )?;

        let partial_token_transaction_proto =
            marshal_token_transaction(&partial_token_transaction, false).map_err(|e| {
                SparkServiceError::InvalidData(format!("Failed to marshal partial token transaction: {:?}", e))
            })?;

        let partial_token_transaction_hash = self
            .proto_hasher
            .hash_proto(partial_token_transaction_proto.to_dynamic().map_err(|e| {
                SparkServiceError::HashError(format!("Failed to hash partial token transaction: {:?}", e))
            })?)
            .map_err(|e| SparkServiceError::HashError(format!("Failed to hash partial token transaction: {:?}", e)))?;

        let signature = self
            .frost_aggregator
            .run_signing_flow(
                musig_id.clone(),
                partial_token_transaction_hash.as_ref(),
                create_signing_metadata(partial_token_transaction.clone(), transaction_type.clone(), true)?,
                nonce_tweak,
            )
            .await
            .map_err(|e| SparkServiceError::FrostAggregatorError(e.to_string()))?;

        let response = self
            .spark_client
            .start_token_transaction(
                StartTransactionRequest {
                    identity_public_key: identity_public_key.serialize().to_vec(),
                    partial_token_transaction: Some(partial_token_transaction_proto),
                    partial_token_transaction_owner_signatures: vec![SignatureWithIndex {
                        signature: signature
                            .serialize()
                            .map_err(|e| {
                                SparkServiceError::DecodeError(format!("Failed to serialize signature: {:?}", e))
                            })?
                            .to_vec(),
                        input_index: 0,
                    }],
                    validity_duration_seconds: DEFAULT_VALIDITY_DURATION_SECONDS,
                },
                identity_public_key,
            )
            .await
            .map_err(|e| SparkServiceError::SparkClientError(e.to_string()))?;

        tracing::debug!("Transaction started");

        // ----- Finalize the transaction -----

        tracing::debug!("Finalize the transaction");

        let final_token_transaction_proto = response.final_token_transaction.ok_or(SparkServiceError::DecodeError(
            "Final token transaction is not found".to_string(),
        ))?;

        let final_token_transaction =
            unmarshal_token_transaction(final_token_transaction_proto.clone()).map_err(|e| {
                SparkServiceError::DecodeError(format!("Failed to unmarshal final token transaction: {:?}", e))
            })?;

        let final_token_transaction_hash = self
            .proto_hasher
            .hash_proto(final_token_transaction_proto.to_dynamic().map_err(|e| {
                SparkServiceError::HashError(format!("Failed to hash final token transaction: {:?}", e))
            })?)
            .map_err(|e| SparkServiceError::HashError(format!("Failed to hash final token transaction: {:?}", e)))?;

        let mut join_handles = vec![];

        for operator_public_key in self.spark_operator_identity_public_keys.clone() {
            let operator_specific_signable_payload = hash_operator_specific_signable_payload(
                final_token_transaction_hash,
                operator_public_key,
            )
            .map_err(|err| {
                SparkServiceError::HashError(format!("Failed to hash operator specific signable payload: {:?}", err))
            })?;

            let musig_id = musig_id.clone();
            let final_token_transaction = final_token_transaction.clone();
            let transaction_type = transaction_type.clone();

            let join_handle = async move {
                let signature = self
                    .frost_aggregator
                    .run_signing_flow(
                        musig_id,
                        operator_specific_signable_payload.as_ref(),
                        create_signing_metadata(final_token_transaction, transaction_type, false)?,
                        nonce_tweak,
                    )
                    .await
                    .map_err(|e| SparkServiceError::FrostAggregatorError(e.to_string()))?;

                let ttxo_signatures = InputTtxoSignaturesPerOperator {
                    ttxo_signatures: vec![SignatureWithIndex {
                        signature: signature
                            .serialize()
                            .map_err(|e| {
                                SparkServiceError::DecodeError(format!("Failed to serialize signature: {:?}", e))
                            })?
                            .to_vec(),
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

        tracing::debug!("Sending commit transaction");

        let response = self
            .spark_client
            .commit_token_transaction(
                CommitTransactionRequest {
                    final_token_transaction: Some(final_token_transaction_proto),
                    final_token_transaction_hash: final_token_transaction_hash.to_byte_array().to_vec(),
                    input_ttxo_signatures_per_operator: signatures,
                    owner_identity_public_key: identity_public_key.serialize().to_vec(),
                },
                identity_public_key,
            )
            .await
            .map_err(|e| SparkServiceError::SparkClientError(e.to_string()))?;

        tracing::debug!("Commit transaction response: {:?}", response);

        tracing::debug!("Transaction committed: {}", final_token_transaction_hash);

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
