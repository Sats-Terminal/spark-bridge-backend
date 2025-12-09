use super::amount_utils::normalize_rune_amount;
use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::IssueSparkDepositAddressRequest;
use frost::utils::convert_public_key_package;
use frost::utils::generate_tweak_bytes;
use gateway_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, DepositStatus, InnerAddress, VerifiersResponses,
};
use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
use gateway_local_db_store::schemas::user_identifier::UserIdentifierStorage;
use global_utils::conversion::convert_network_to_spark_network;
use spark_address::{SparkAddressData, encode_spark_address};
use tracing::instrument;

#[instrument(skip(flow_router), level = "trace", ret)]
pub async fn handle(
    flow_router: &mut FlowProcessorRouter,
    request: IssueSparkDepositAddressRequest,
) -> Result<String, FlowProcessorError> {
    let IssueSparkDepositAddressRequest {
        user_id,
        rune_id,
        amount,
    } = request;
    let user_id_str = user_id.to_string();
    tracing::info!("Handling spark addr issuing for user id: {:?}", user_id_str.clone());
    let local_db_storage = flow_router.storage.clone();

    let maybe_existing = flow_router
        .storage
        .get_row_by_user_id(user_id.clone(), &rune_id)
        .await?;

    let dkg_share_id = if let Some(user_ids) = maybe_existing {
        user_ids.dkg_share_id
    } else {
        let user_ids = flow_router.storage.get_random_unused_dkg_share(&rune_id, false).await?;
        flow_router
            .storage
            .set_external_user_id(user_ids.dkg_share_id, &user_id)
            .await?;
        user_ids.dkg_share_id
    };

    let nonce = generate_tweak_bytes();
    let public_key_package = flow_router
        .frost_aggregator
        .get_public_key_package(dkg_share_id, Some(nonce))
        .await
        .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to get public key package: {}", e)))?;
    let public_key = convert_public_key_package(&public_key_package)
        .map_err(|e| FlowProcessorError::InvalidDataError(e.to_string()))?;

    let address = encode_spark_address(SparkAddressData {
        identity_public_key: public_key.to_string(),
        network: convert_network_to_spark_network(flow_router.network),
        invoice: None,
        signature: None,
    })
    .map_err(|e| FlowProcessorError::InvalidDataError(e.to_string()))?;
    let verifiers_responses = VerifiersResponses::new(
        DepositStatus::Created,
        flow_router.verifier_configs.iter().map(|v| v.id).collect(),
    );

    let normalized_amount = normalize_rune_amount(amount, &rune_id, &flow_router.rune_metadata_client).await?;

    local_db_storage
        .insert_deposit_addr_info(DepositAddrInfo {
            dkg_share_id,
            nonce,
            deposit_address: InnerAddress::SparkAddress(address.clone()),
            bridge_address: None,
            is_btc: false,
            amount: normalized_amount,
            requested_amount: amount,
            confirmation_status: verifiers_responses,
        })
        .await?;

    tracing::info!("Spark addr issuing completed for user id: {:?}", user_id_str);

    Ok(address)
}
