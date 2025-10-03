use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::BridgeRunesRequest;
use bitcoin::secp256k1::PublicKey;
use frost::traits::AggregatorDkgShareStorage;
use frost::utils::generate_issuer_public_key;
use gateway_local_db_store::schemas::deposit_address::{DepositAddressStorage, InnerAddress};
use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
use gateway_local_db_store::schemas::user_identifier::{
    UserIdentifier, UserIdentifierData, UserIdentifierStorage, UserIds, UserUniqueId,
};
use gateway_spark_service::types::SparkTransactionType;
use gateway_spark_service::utils::{convert_network_to_spark_network, create_wrunes_metadata};
use spark_address::{SparkAddressData, encode_spark_address};
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
        .get_row_by_deposit_address(InnerAddress::BitcoinAddress(request.btc_address.clone()))
        .await
        .map_err(FlowProcessorError::DbError)?
        .ok_or(FlowProcessorError::InvalidDataError(
            "Deposit address info not found".to_string(),
        ))?;
    let user_info = flow_router
        .storage
        .get_row_by_user_unique_id(&UserUniqueId {
            uuid: deposit_addr_info.user_uuid,
            rune_id: deposit_addr_info.rune_id,
        })
        .await
        .map_err(FlowProcessorError::DbError)?
        .ok_or(FlowProcessorError::InvalidDataError("User info not found".to_string()))?;
    let rune_id = user_info.rune_id;

    let possible_issuer_user_ids = flow_router
        .storage
        .get_issuer_ids(rune_id.clone())
        .await
        .map_err(FlowProcessorError::DbError)?;

    let issuer_user_ids = match possible_issuer_user_ids {
        Some(user_ids) => user_ids,
        None => {
            tracing::debug!("Issuer musig id not found, running dkg for issuer ...");
            let issuer_public_key = generate_issuer_public_key();

            let issuer_ids = flow_router
                .storage
                .get_random_unused_dkg_share(UserIdentifierData {
                    rune_id: rune_id.clone(),
                    public_key: issuer_public_key.to_string(),
                    is_issuer: true,
                })
                .await?;

            // Dkg flow has to be already completed

            let wrunes_metadata = create_wrunes_metadata(rune_id.clone(), issuer_public_key, flow_router.network)?;

            flow_router
                .spark_service
                .send_spark_transaction(
                    issuer_ids.dkg_share_id,
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

            UserIds {
                user_uuid: issuer_ids.user_uuid,
                dkg_share_id: issuer_ids.dkg_share_id,
                rune_id: rune_id.clone(),
            }
        }
    };

    let issuer_public_key_package = flow_router
        .frost_aggregator
        .get_public_key_package(issuer_user_ids.dkg_share_id, None)
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
    let _user_identifier = flow_router
        .storage
        .get_row_by_user_unique_id(&UserUniqueId {
            uuid: deposit_addr_info.user_uuid,
            rune_id,
        })
        .await
        .map_err(FlowProcessorError::DbError)?
        .ok_or(FlowProcessorError::InvalidDataError("User info not found".to_string()))?;

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
