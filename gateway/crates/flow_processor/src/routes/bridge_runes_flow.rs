use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::BridgeRunesRequest;
use bitcoin::secp256k1::PublicKey;
use gateway_local_db_store::schemas::deposit_address::{DepositAddressStorage, InnerAddress};
use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
use gateway_local_db_store::schemas::user_identifier::{UserIdentifierStorage, UserIds};
use gateway_spark_service::types::SparkTransactionType;
use gateway_spark_service::utils::create_wrunes_metadata;
use global_utils::conversion::convert_network_to_spark_network;
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

    let wrunes_metadata = create_wrunes_metadata(
        &rune_id,
        issuer_musig_public_key,
        flow_router.network,
        &flow_router.bitcoin_indexer,
    )
    .await?;

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
            convert_network_to_spark_network(flow_router.network),
        )
        .await
        .map_err(|e| FlowProcessorError::SparkServiceError(format!("Failed to send spark mint transaction: {}", e)))?;

    tracing::info!("Bridge runes flow completed for address: {}", request.btc_address);

    Ok(())
}
