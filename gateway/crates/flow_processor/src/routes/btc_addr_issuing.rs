use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::DkgFlowRequest;
use bitcoin::{PublicKey, secp256k1};
use frost::utils::convert_public_key_package;
use tracing::info;

const LOG_PATH: &str = "flow_processor:routes:btc_addr_issuing";

pub async fn handle(
    flow_processor: &mut FlowProcessorRouter,
    request: DkgFlowRequest,
) -> Result<secp256k1::PublicKey, FlowProcessorError> {
    info!("[LOG_PATH] Handling btc addr issuing ...");

    let public_key_package = flow_processor
        .frost_aggregator
        .run_dkg_flow(request.musig_id)
        .await
        .map_err(|e| FlowProcessorError::FrostAggregatorError(e.to_string()))?;

    let public_key = convert_public_key_package(public_key_package)
        .map_err(|e| FlowProcessorError::InvalidDataError(e.to_string()))?;

    Ok(public_key)
}
