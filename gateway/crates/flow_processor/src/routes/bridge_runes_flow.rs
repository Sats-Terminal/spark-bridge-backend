use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::BridgeRunesRequest;
use bitcoin::secp256k1::PublicKey;
use frost::traits::AggregatorMusigIdStorage;
use frost::types::MusigId;
use frost::utils::generate_issuer_public_key;
use gateway_local_db_store::schemas::deposit_address::{DepositAddressStorage, InnerAddress};
use gateway_spark_service::types::SparkTransactionType;
use gateway_spark_service::utils::{convert_network_to_spark_network, create_wrunes_metadata};
use tracing::instrument;

#[instrument(skip(flow_router), level = "trace", ret)]
pub async fn handle(
    flow_router: &mut FlowProcessorRouter,
    request: BridgeRunesRequest,
) -> Result<(), FlowProcessorError> {
    tracing::info!("Handling btc addr bridge runes flow for address: {}", request.btc_address);

    let deposit_addr_info = flow_router
        .storage
        .get_row_by_deposit_address(InnerAddress::BitcoinAddress(request.btc_address.clone()))
        .await
        .map_err(FlowProcessorError::DbError)?
        .ok_or(FlowProcessorError::InvalidDataError(
            "Deposit address info not found".to_string(),
        ))?;

    let rune_id = deposit_addr_info.musig_id.get_rune_id();

    let issuer_musig_id = flow_router
        .storage
        .get_issuer_musig_id(rune_id.clone())
        .await
        .map_err(FlowProcessorError::DbError)?;

    let issuer_musig_id = match issuer_musig_id {
        Some(issuer_musig_id) => issuer_musig_id,
        None => {
            tracing::debug!("Issuer musig id not found, running dkg for issuer ...");
            let issuer_public_key = generate_issuer_public_key();

            let musig_id = MusigId::Issuer {
                issuer_public_key,
                rune_id: rune_id.clone(),
            };

            let issuer_public_key_package =
                flow_router
                    .frost_aggregator
                    .run_dkg_flow(&musig_id)
                    .await
                    .map_err(|e| {
                        FlowProcessorError::FrostAggregatorError(format!("Failed to run DKG flow for issuer: {}", e))
                    })?;

            let issuer_musig_public_key_bytes = issuer_public_key_package.verifying_key().serialize().map_err(|e| {
                FlowProcessorError::InvalidDataError(format!("Failed to serialize issuer musig public key: {}", e))
            })?;
            let issuer_musig_public_key = PublicKey::from_slice(&issuer_musig_public_key_bytes)?;

            let wrunes_metadata =
                create_wrunes_metadata(rune_id.clone(), issuer_musig_public_key, flow_router.network)?;

            flow_router
                .spark_service
                .send_spark_transaction(
                    musig_id.clone(),
                    None,
                    wrunes_metadata.token_identifier,
                    SparkTransactionType::Create {
                        token_name: wrunes_metadata.token_name,
                        token_ticker: wrunes_metadata.token_ticker,
                    },
                    convert_network_to_spark_network(flow_router.network),
                )
                .await
                .map_err(|e| {
                    FlowProcessorError::SparkServiceError(format!("Failed to send spark create transaction: {}", e))
                })?;

            musig_id
        }
    };

    let issuer_public_key_package = flow_router
        .frost_aggregator
        .get_public_key_package(issuer_musig_id.clone(), None)
        .await
        .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to get public key package: {}", e)))?;

    let issuer_musig_public_key_bytes = issuer_public_key_package.verifying_key().serialize().map_err(|e| {
        FlowProcessorError::InvalidDataError(format!("Failed to serialize issuer musig public key: {}", e))
    })?;
    let issuer_musig_public_key = PublicKey::from_slice(&issuer_musig_public_key_bytes)?;

    let wrunes_metadata = create_wrunes_metadata(rune_id.clone(), issuer_musig_public_key, flow_router.network)?;

    let deposit_addr_info = flow_router
        .storage
        .get_row_by_deposit_address(InnerAddress::BitcoinAddress(request.btc_address.clone()))
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
            issuer_musig_id.clone(),
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
