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
    let IssueBtcDepositAddressRequest {
        user_id,
        rune_id,
        amount,
    } = request;
    tracing::info!("Handling btc addr issuing for musig id: {:?}", user_id);
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
        .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to convert public key package: {}", e)))?;
    let address = get_tweaked_p2tr_address(public_key, nonce, flow_router.network)
        .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to create address: {}", e)))?;

    let verifiers_responses = VerifiersResponses::new(
        DepositStatus::Created,
        flow_router.verifier_configs.iter().map(|v| v.id).collect(),
    );

    let normalized_amount = normalize_rune_amount(amount, &rune_id, &flow_router.rune_metadata_client).await?;

    local_db_storage
        .insert_deposit_addr_info(DepositAddrInfo {
            dkg_share_id,
            nonce,
            deposit_address: InnerAddress::BitcoinAddress(address.clone()),
            bridge_address: None,
            is_btc: true,
            amount: normalized_amount,
            requested_amount: amount,
            confirmation_status: verifiers_responses,
        })
        .await?;

    Ok(address)
}
