use super::amount_utils::normalize_rune_amount;
use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::IssueBtcDepositAddressRequest;
use bitcoin::Address;
use frost::utils::convert_public_key_package;
use frost::utils::{generate_tweak_bytes, get_tweaked_p2tr_address};
use gateway_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, DepositStatus, InnerAddress, VerifiersResponses,
};
use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
use gateway_local_db_store::schemas::user_identifier::UserIdentifierStorage;
use tracing::instrument;

#[instrument(skip(flow_router), level = "trace", ret)]
pub async fn handle(
    flow_router: &mut FlowProcessorRouter,
    request: IssueBtcDepositAddressRequest,
) -> Result<Address, FlowProcessorError> {
    tracing::info!("Handling btc addr issuing for musig id: {:?}", request.user_id);
    let local_db_storage = flow_router.storage.clone();

    let dkg_share_id = match flow_router
        .storage
        .get_row_by_user_id(request.user_id, &request.rune_id)
        .await?
    {
        Some(user_ids) => user_ids.dkg_share_id,
        None => {
            flow_router
                .storage
                .get_random_unused_dkg_share(&request.rune_id, false)
                .await?
                .dkg_share_id
        }
    };

    let nonce = generate_tweak_bytes();
    let public_key_package = flow_router
        .frost_aggregator
        .get_public_key_package(dkg_share_id, Some(nonce))
        .await
        .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to get public key package: {}", e)))?;
    let public_key = convert_public_key_package(&public_key_package)
        .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to convert public key package: {}", e)))?;
    let address = get_tweaked_p2tr_address(public_key, nonce, flow_router.network)
        .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to create address: {}", e)))?;

    let verifiers_responses = VerifiersResponses::new(
        DepositStatus::Created,
        flow_router.verifier_configs.iter().map(|v| v.id).collect(),
    );

    let normalized_amount =
        normalize_rune_amount(request.amount, &request.rune_id, &flow_router.rune_metadata_client).await?;

    local_db_storage
        .insert_deposit_addr_info(DepositAddrInfo {
            dkg_share_id,
            nonce,
            deposit_address: InnerAddress::BitcoinAddress(address.clone()),
            bridge_address: None,
            is_btc: true,
            amount: normalized_amount,
            confirmation_status: verifiers_responses,
        })
        .await?;

    Ok(address)
}
