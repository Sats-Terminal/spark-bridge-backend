use std::sync::Arc;

use bitcoin::{
    hashes::{Hash, HashEngine, sha256, sha256::Hash as Sha256Hash},
    secp256k1::PublicKey,
};
use frost::aggregator::FrostAggregator;
use frost::types::{SigningMetadata, TweakBytes};
use frost_secp256k1_tr::Signature;
use futures::future::join_all;
use global_utils::conversion::spark_network_to_proto_network;
use k256::elliptic_curve::{point::AffineCoordinates, sec1::FromEncodedPoint};
use k256::{AffinePoint, EncodedPoint};
use lrc20::marshal::{marshal_token_transaction, unmarshal_token_transaction};
use proto_hasher::ProtoHasher;
use spark_address::Network;
use spark_client::client::SparkRpcClient;
use spark_protos::{
    prost::Message,
    reflect::ToDynamicMessage,
    spark_authn::{GetChallengeRequest, VerifyChallengeRequest},
    spark_token::{
        CommitTransactionRequest, InputTtxoSignaturesPerOperator, SignatureWithIndex, StartTransactionRequest,
    },
};
use token_identifier::TokenIdentifier;
use uuid::Uuid;

use crate::{
    errors::SparkServiceError,
    types::{create_partial_token_transaction, *},
};

const DEFAULT_VALIDITY_DURATION_SECONDS: u64 = 300;

pub struct SparkService {
    spark_client: SparkRpcClient,
    frost_aggregator: Arc<FrostAggregator>,
    proto_hasher: ProtoHasher,
    spark_operator_identity_public_keys: Vec<PublicKey>,
}

impl SparkService {
    pub fn new(
        spark_client: SparkRpcClient,
        frost_aggregator: Arc<FrostAggregator>,
        spark_operator_identity_public_keys: Vec<PublicKey>,
    ) -> Self {
        Self {
            spark_client,
            frost_aggregator,
            proto_hasher: ProtoHasher::new(),
            spark_operator_identity_public_keys,
        }
    }

    #[tracing::instrument(level = "trace", skip(self), ret)]
    async fn get_musig_public_key(
        &self,
        issuer_dkg_share_id: Uuid,
        nonce_tweak: Option<TweakBytes>,
    ) -> Result<PublicKey, SparkServiceError> {
        let public_key_package = self
            .frost_aggregator
            .get_public_key_package(issuer_dkg_share_id, nonce_tweak)
            .await
            .map_err(|err| {
                SparkServiceError::FrostAggregatorError(format!("Failed to get public key package, err: {err}"))
            })?;

        let issuer_public_key_bytes = public_key_package
            .verifying_key()
            .serialize()
            .map_err(|err| SparkServiceError::InvalidData(format!("Failed to serialize public key: {:?}", err)))?;

        let issuer_public_key = PublicKey::from_slice(&issuer_public_key_bytes)
            .map_err(|err| SparkServiceError::DecodeError(format!("Failed to parse public key: {:?}", err)))?;

        Ok(issuer_public_key)
    }

    #[tracing::instrument(level = "trace", skip(self), ret)]
    async fn authenticate(
        &self,
        issuer_dkg_share_id: Uuid,
        nonce_tweak: Option<TweakBytes>,
    ) -> Result<(), SparkServiceError> {
        tracing::debug!(
            "Authenticating with issuer_dkg_share_id: {:?}, nonce tweak: {:?}",
            issuer_dkg_share_id,
            nonce_tweak
        );
        let identity_public_key = self.get_musig_public_key(issuer_dkg_share_id, nonce_tweak).await?;

        let session_token = self.spark_client.get_auth_session(identity_public_key).await;
        if session_token.is_some() {
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
                issuer_dkg_share_id,
                message_hash.as_byte_array().as_slice(),
                SigningMetadata::Authorization,
                nonce_tweak,
                false,
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

        tracing::debug!("Challenge verified for issuer_dkg_share_id: {:?}", issuer_dkg_share_id);

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self), ret)]
    pub async fn send_spark_transaction(
        &self,
        issuer_dkg_share_id: Uuid,
        nonce_tweak: Option<TweakBytes>,
        token_identifier: TokenIdentifier,
        transaction_type: SparkTransactionType,
        network: Network,
    ) -> Result<(), SparkServiceError> {
        tracing::debug!(
            "Send spark transaction with issuer_dkg_share_id: {:?}, nonce tweak: {:?}, token identifier: {:?}",
            issuer_dkg_share_id,
            nonce_tweak,
            token_identifier.to_string()
        );

        self.authenticate(issuer_dkg_share_id, nonce_tweak).await?;

        tracing::debug!("Network: {:?}", network);

        let identity_public_key = self.get_musig_public_key(issuer_dkg_share_id, nonce_tweak).await?;

        // ----- Start the transaction -----

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

        let signing_metadata =
            create_signing_metadata(partial_token_transaction.clone(), transaction_type.clone(), true)?;
        tracing::info!(
            "SparkService signing partial tx hash (round1/2): issuer_dkg_share_id={}, nonce_tweak_present={}, tx_type={:?}, token_identifier={}",
            issuer_dkg_share_id,
            nonce_tweak.is_some(),
            transaction_type,
            token_identifier.to_string()
        );
        let signature = self
            .frost_aggregator
            .run_signing_flow(
                issuer_dkg_share_id,
                partial_token_transaction_hash.as_ref(),
                signing_metadata,
                nonce_tweak,
                false,
            )
            .await
            .map_err(|e| SparkServiceError::FrostAggregatorError(e.to_string()))?;
        let owner_signature_bytes = serialize_frost_signature_bip340(&signature)?;
        tracing::debug!(
            "Owner signature (partial) len={} hex={}",
            owner_signature_bytes.len(),
            hex::encode(&owner_signature_bytes)
        );

        tracing::info!(
            "SparkService starting token transaction: issuer_dkg_share_id={}, nonce_tweak_present={}, tx_type={:?}, token_identifier={}",
            issuer_dkg_share_id,
            nonce_tweak.is_some(),
            transaction_type,
            token_identifier.to_string()
        );
        let response = self
            .spark_client
            .start_token_transaction(
                StartTransactionRequest {
                    identity_public_key: identity_public_key.serialize().to_vec(),
                    partial_token_transaction: Some(partial_token_transaction_proto),
                    partial_token_transaction_owner_signatures: vec![SignatureWithIndex {
                        signature: owner_signature_bytes,
                        input_index: 0,
                    }],
                    validity_duration_seconds: DEFAULT_VALIDITY_DURATION_SECONDS,
                },
                identity_public_key,
            )
            .await
            .map_err(|e| {
                tracing::error!(
                    "SparkService start_token_transaction failed: issuer_dkg_share_id={}, nonce_tweak_present={}, tx_type={:?}, token_identifier={}, err={}",
                    issuer_dkg_share_id,
                    nonce_tweak.is_some(),
                    transaction_type,
                    token_identifier.to_string(),
                    e
                );
                SparkServiceError::SparkClientError(e.to_string())
            })?;

        tracing::debug!(
            "Transaction: {:?} for dkg share id: {:?}, with token identifier: {:?}, started",
            transaction_type,
            issuer_dkg_share_id,
            token_identifier.to_string()
        );

        // ----- Finalize the transaction -----

        tracing::debug!(
            "Finalize the transaction: {:?}, for dkg share id: {:?}, with token identifier: {:?}",
            transaction_type,
            issuer_dkg_share_id,
            token_identifier.to_string()
        );

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
            tracing::info!(
                "SparkService signing operator-specific payload: issuer_dkg_share_id={}, operator_pk={}",
                issuer_dkg_share_id,
                hex::encode(operator_public_key.serialize())
            );
            let operator_specific_signable_payload = hash_operator_specific_signable_payload(
                final_token_transaction_hash,
                operator_public_key,
            )
            .map_err(|err| {
                SparkServiceError::HashError(format!("Failed to hash operator specific signable payload: {:?}", err))
            })?;

            let final_token_transaction = final_token_transaction.clone();
            let transaction_type = transaction_type.clone();

            let join_handle = async move {
                let signature = self
                    .frost_aggregator
                    .run_signing_flow(
                        issuer_dkg_share_id,
                        operator_specific_signable_payload.as_ref(),
                        create_signing_metadata(final_token_transaction, transaction_type, false)?,
                        nonce_tweak,
                        false,
                    )
                    .await
                    .map_err(|e| SparkServiceError::FrostAggregatorError(e.to_string()))?;

                let ttxo_signatures = InputTtxoSignaturesPerOperator {
                    ttxo_signatures: vec![SignatureWithIndex {
                        signature: serialize_frost_signature_bip340(&signature)?,
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

        tracing::debug!(
            "Sending commit transaction: {:?}, for dkg share id: {:?}, with token identifier: {:?}",
            transaction_type,
            issuer_dkg_share_id,
            token_identifier.to_string()
        );

        tracing::info!(
            "SparkService committing token transaction: issuer_dkg_share_id={}, tx_type={:?}, token_identifier={}",
            issuer_dkg_share_id,
            transaction_type,
            token_identifier.to_string()
        );
        let _response = self
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
            .map_err(|e| {
                tracing::error!(
                    "SparkService commit_token_transaction failed: issuer_dkg_share_id={}, nonce_tweak_present={}, tx_type={:?}, token_identifier={}, err={}",
                    issuer_dkg_share_id,
                    nonce_tweak.is_some(),
                    transaction_type,
                    token_identifier.to_string(),
                    e
                );
                SparkServiceError::SparkClientError(e.to_string())
            })?;

        tracing::info!(
            "Transaction: {:?} for dkg share id: {:?}, with token identifier: {:?}, committed",
            transaction_type,
            issuer_dkg_share_id,
            token_identifier.to_string()
        );

        Ok(())
    }
}

fn serialize_frost_signature_bip340(signature: &Signature) -> Result<Vec<u8>, SparkServiceError> {
    // frost-secp256k1-tr already serializes signatures as BIP-340 (rx || z) 64 bytes.
    // If we ever see a compressed R + z (65 bytes), convert to BIP-340 form.
    let sig_bytes = signature.serialize().map_err(|e| {
        SparkServiceError::DecodeError(format!("Failed to serialize FROST signature: {:?}", e))
    })?;

    match sig_bytes.len() {
        64 => Ok(sig_bytes),
        65 => {
            let (r_bytes, z_bytes) = sig_bytes.split_at(33);
            let encoded_point = EncodedPoint::from_bytes(r_bytes).map_err(|e| {
                SparkServiceError::DecodeError(format!("Failed to parse R point from signature: {:?}", e))
            })?;
            let r_affine = AffinePoint::from_encoded_point(&encoded_point)
                .into_option()
                .ok_or_else(|| SparkServiceError::DecodeError("Invalid R point in serialized signature".to_string()))?;
            let rx = r_affine.x();

            let mut out = Vec::with_capacity(64);
            out.extend_from_slice(rx.as_ref());
            out.extend_from_slice(z_bytes);
            Ok(out)
        }
        other => Err(SparkServiceError::DecodeError(format!(
            "Unexpected FROST signature length: {}",
            other
        ))),
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
