use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::rune_metadata_client::{RuneMetadata, RuneMetadataClient};
use crate::types::BridgeRunesRequest;
use bitcoin::secp256k1::PublicKey;
use frost::utils::generate_issuer_public_key;
use gateway_local_db_store::schemas::deposit_address::{DepositAddressStorage, InnerAddress};
use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
use gateway_local_db_store::schemas::rune_metadata::RuneMetadataStorage;
use gateway_local_db_store::schemas::user_identifier::{UserIdentifierStorage, UserIds};
use gateway_spark_service::types::SparkTransactionType;
use gateway_spark_service::utils::{RuneTokenConfig, create_wrunes_metadata};
use global_utils::conversion::convert_network_to_spark_network;
use serde_json;
use std::sync::Arc;
use tracing::instrument;

#[instrument(skip(flow_router), level = "trace", ret)]
pub async fn handle(
    flow_router: &mut FlowProcessorRouter,
    request: BridgeRunesRequest,
) -> Result<(), FlowProcessorError> {
    tracing::info!(
        "Handling btc addr bridge runes flow for address: {}",
        request.btc_address
    );

    let deposit_addr_info = flow_router
        .storage
        .get_row_by_deposit_address(&InnerAddress::BitcoinAddress(request.btc_address.clone()))
        .await
        .map_err(FlowProcessorError::DbError)?
        .ok_or(FlowProcessorError::InvalidDataError(
            "Deposit address info not found".to_string(),
        ))?;
    let user_info = flow_router
        .storage
        .get_row_by_dkg_id(deposit_addr_info.dkg_share_id)
        .await
        .map_err(FlowProcessorError::DbError)?
        .ok_or(FlowProcessorError::InvalidDataError("User info not found".to_string()))?;
    let rune_id = user_info.rune_id;

    let possible_issuer_user_ids = flow_router
        .storage
        .get_issuer_ids(&rune_id)
        .await
        .map_err(FlowProcessorError::DbError)?;

    let issuer_user_ids = match possible_issuer_user_ids {
        Some(user_ids) => user_ids,
        None => {
            tracing::debug!("Issuer musig id not found, running dkg for issuer ...");
            let issuer_ids = flow_router.storage.get_random_unused_dkg_share(&rune_id, true).await?;

            // Dkg flow has to be already completed

            let issuer_public_key_package = flow_router
                .frost_aggregator
                .get_public_key_package(issuer_ids.dkg_share_id, None)
                .await
                .map_err(|e| {
                    FlowProcessorError::FrostAggregatorError(format!("Failed to get public key package, err: {}", e))
                })?;

            let issuer_musig_public_key_bytes = issuer_public_key_package.verifying_key().serialize().map_err(|e| {
                FlowProcessorError::InvalidDataError(format!("Failed to serialize issuer musig public key: {}", e))
            })?;
            let issuer_musig_public_key = PublicKey::from_slice(&issuer_musig_public_key_bytes)?;

            let wrunes_metadata = create_wrunes_metadata(
                &rune_id,
                issuer_musig_public_key,
                flow_router.network,
                &flow_router.bitcoin_indexer,
            )
            .await?;

            flow_router
                .spark_service
                .send_spark_transaction(
                    issuer_ids.dkg_share_id,
                    None,
                    wrunes_metadata.token_identifier,
                    SparkTransactionType::Create { wrunes_metadata },
                    convert_network_to_spark_network(flow_router.network),
                )
                .await
                .map_err(|e| {
                    FlowProcessorError::SparkServiceError(format!("Failed to send spark create transaction: {}", e))
                })?;

            UserIds {
                user_id: issuer_ids.user_id,
                dkg_share_id: issuer_ids.dkg_share_id,
                rune_id: rune_id.clone(),
                is_issuer: true,
            }
        }
    };

    let issuer_public_key_package = flow_router
        .frost_aggregator
        .get_public_key_package(issuer_user_ids.dkg_share_id, None)
        .await
        .map_err(|e| {
            FlowProcessorError::FrostAggregatorError(format!("Failed to get public key package, err: {}", e))
        })?;

    let issuer_musig_public_key_bytes = issuer_public_key_package.verifying_key().serialize().map_err(|e| {
        FlowProcessorError::InvalidDataError(format!("Failed to serialize issuer musig public key: {}", e))
    })?;
    let issuer_musig_public_key = PublicKey::from_slice(&issuer_musig_public_key_bytes)?;

    let rune_metadata = fetch_rune_metadata(&flow_router.rune_metadata_client, &rune_id).await;
    let rune_token_config = build_rune_token_config(&rune_id, rune_metadata.as_ref());
    let wrunes_metadata = create_wrunes_metadata(&rune_token_config, issuer_musig_public_key, flow_router.network)
        .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to create wrunes metadata: {}", e)))?;

    let deposit_addr_info = flow_router
        .storage
        .get_row_by_deposit_address(&InnerAddress::BitcoinAddress(request.btc_address.clone()))
        .await
        .map_err(FlowProcessorError::DbError)?
        .ok_or(FlowProcessorError::InvalidDataError(
            "Deposit address info not found".to_string(),
        ))?;

    let bridge_address = deposit_addr_info
        .bridge_address
        .ok_or(FlowProcessorError::InvalidDataError(
            "Bridge address not found".to_string(),
        ))?;

    let spark_network = convert_network_to_spark_network(flow_router.network);
    cache_wrune_metadata(
        &flow_router.storage,
        &rune_id,
        rune_metadata.as_ref(),
        &wrunes_metadata,
        &issuer_musig_public_key,
        flow_router.network,
        spark_network,
    )
    .await;

    flow_router
        .spark_service
        .send_spark_transaction(
            issuer_user_ids.dkg_share_id,
            None,
            wrunes_metadata.token_identifier,
            SparkTransactionType::Mint {
                receiver_spark_address: bridge_address.to_string(),
                token_amount: deposit_addr_info.amount,
            },
            spark_network,
        )
        .await
        .map_err(|e| FlowProcessorError::SparkServiceError(format!("Failed to send spark mint transaction: {}", e)))?;

    tracing::info!("Bridge runes flow completed for address: {}", request.btc_address);

    Ok(())
}

async fn fetch_rune_metadata(client: &Option<Arc<RuneMetadataClient>>, rune_id: &str) -> Option<RuneMetadata> {
    match client {
        Some(client) => match client.get_metadata(rune_id).await {
            Ok(metadata) => Some(metadata),
            Err(err) => {
                tracing::warn!("Failed to fetch rune metadata for {}: {}", rune_id, err);
                None
            }
        },
        None => None,
    }
}

fn build_rune_token_config(rune_id: &str, metadata: Option<&RuneMetadata>) -> RuneTokenConfig {
    RuneTokenConfig {
        rune_id: rune_id.to_string(),
        rune_name: metadata.map(|m| m.name.clone()),
        divisibility: metadata.map(|m| m.divisibility),
        max_supply: metadata.and_then(|m| m.max_supply),
        icon_url: metadata.and_then(|m| m.icon_url.clone()),
    }
}

async fn cache_wrune_metadata(
    storage: &Arc<gateway_local_db_store::storage::LocalDbStorage>,
    rune_id: &str,
    rune_metadata: Option<&RuneMetadata>,
    wrune_metadata: &gateway_spark_service::utils::WRunesMetadata,
    issuer_public_key: &PublicKey,
    bitcoin_network: bitcoin::Network,
    spark_network: spark_address::Network,
) {
    let rune_metadata_value = match rune_metadata {
        Some(metadata) => match serde_json::to_value(metadata) {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::warn!("Failed to serialize rune metadata for {}: {}", rune_id, err);
                return;
            }
        },
        None => None,
    };
    let wrune_metadata_value = match serde_json::to_value(wrune_metadata) {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!("Failed to serialize wRune metadata for {}: {}", rune_id, err);
            return;
        }
    };

    if let Err(err) = storage
        .upsert_rune_metadata(
            rune_id.to_string(),
            rune_metadata_value,
            wrune_metadata_value,
            issuer_public_key.to_string(),
            bitcoin_network.to_string(),
            format!("{:?}", spark_network),
        )
        .await
    {
        tracing::warn!("Failed to persist rune metadata for {}: {}", rune_id, err);
    }
}
