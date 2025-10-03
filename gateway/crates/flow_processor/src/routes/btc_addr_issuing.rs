use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::IssueBtcDepositAddressRequest;
use bitcoin::Address;
use frost::traits::AggregatorMusigIdStorage;
use frost::types::AggregatorDkgState;
use frost::utils::convert_public_key_package;
use frost::utils::{generate_tweak_bytes, get_tweaked_p2tr_address};
use gateway_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, DepositStatus, InnerAddress, VerifiersResponses,
};
use tracing::instrument;

#[instrument(skip(flow_router), level = "trace", ret)]
pub async fn handle(
    flow_router: &mut FlowProcessorRouter,
    request: IssueBtcDepositAddressRequest,
) -> Result<Address, FlowProcessorError> {
    tracing::info!("Handling btc addr issuing for musig id: {:?}", request.musig_id);

    let public_key_package = match flow_router.storage.get_musig_id_data(&request.musig_id).await? {
        None => {
            tracing::debug!("Missing musig, running dkg from the beginning ...");
            flow_router
                .frost_aggregator
                .run_dkg_flow(&request.musig_id)
                .await
                .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to run DKG flow: {}", e)))?
        }
        Some(x) => {
            // extract data from db, get nonce and generate new one, return it to user
            match x.dkg_state {
                AggregatorDkgState::DkgRound1 { .. } => {
                    return Err(FlowProcessorError::UnfinishedDkgState(
                        "Should be DkgFinalized, got DkgRound1".to_string(),
                    ));
                }
                AggregatorDkgState::DkgRound2 { .. } => {
                    return Err(FlowProcessorError::UnfinishedDkgState(
                        "Should be DkgFinalized, got DkgRound2".to_string(),
                    ));
                }
                AggregatorDkgState::DkgFinalized {
                    public_key_package: pubkey_package,
                } => pubkey_package,
            }
        }
    };

    let nonce = generate_tweak_bytes();
    let public_key = convert_public_key_package(&public_key_package)
        .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to convert public key package: {}", e)))?;
    let address = get_tweaked_p2tr_address(public_key, nonce, flow_router.network)
        .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to create address: {}", e)))?;

    let verifiers_responses = VerifiersResponses::new(
        DepositStatus::Created,
        flow_router.verifier_configs.iter().map(|v| v.id).collect(),
    );

    flow_router.storage
        .set_deposit_addr_info(DepositAddrInfo {
            musig_id: request.musig_id.clone(),
            nonce,
            deposit_address: InnerAddress::BitcoinAddress(address.clone()),
            bridge_address: None,
            is_btc: true,
            amount: request.amount,
            confirmation_status: verifiers_responses,
        })
        .await?;

    Ok(address)
}
