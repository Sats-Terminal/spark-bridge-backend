use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::IssueSparkDepositAddressRequest;
use frost::traits::AggregatorDkgShareStorage;
use frost::types::{AggregatorDkgShareData, AggregatorDkgState};
use frost::utils::convert_public_key_package;
use frost::utils::generate_tweak_bytes;
use gateway_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, DepositStatus, InnerAddress, VerifiersResponses,
};
use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
use gateway_local_db_store::schemas::user_identifier::{UserIdentifierStorage, UserIds};
use global_utils::conversion::convert_network_to_spark_network;
use spark_address::{SparkAddressData, encode_spark_address};
use tracing::instrument;

#[instrument(skip(flow_router), level = "trace", ret)]
pub async fn handle(
    flow_router: &mut FlowProcessorRouter,
    request: IssueSparkDepositAddressRequest,
) -> Result<String, FlowProcessorError> {
    tracing::info!("Handling spark addr issuing for musig id: {:?}", request.user_id);
    let local_db_storage = flow_router.storage.clone();

    let dkg_share_id = match flow_router.storage.get_row_by_user_id(request.user_id, request.rune_id.clone()).await? {
        Some(user_ids) => user_ids.dkg_share_id,
        None => flow_router.storage.get_random_unused_dkg_share(request.rune_id.clone(), false).await?
            .dkg_share_id
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

    local_db_storage
        .insert_deposit_addr_info(DepositAddrInfo {
            dkg_share_id,
            nonce,
            deposit_address: InnerAddress::SparkAddress(address.clone()),
            bridge_address: None,
            is_btc: false,
            amount: request.amount,
            confirmation_status: verifiers_responses,
        })
        .await?;

    tracing::info!("Spark addr issuing completed for musig id: {:?}", request.user_id);

    Ok(address)
}
