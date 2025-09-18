use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::IssueBtcDepositAddressRequest;
use bitcoin::Address;
use frost::traits::AggregatorMusigIdStorage;
use frost::types::AggregatorDkgState;
use frost::utils::convert_public_key_package;
use gateway_local_db_store::schemas::deposit_address::{DepositAddrInfo, DepositAddressStorage, DepositStatus, VerifiersResponses};
use frost::utils::{get_address, generate_nonce};
use tracing;

const LOG_PATH: &str = "flow_processor:routes:btc_addr_issuing";

pub async fn handle(
    flow_processor: &mut FlowProcessorRouter,
    request: IssueBtcDepositAddressRequest,
) -> Result<Address, FlowProcessorError> {
    let local_db_storage = flow_processor.storage.clone();

    let public_key_package = 
        match flow_processor.storage.get_musig_id_data(&request.musig_id).await? {
            None => {
                tracing::debug!("[{LOG_PATH}] Missing musig, running dkg from the beginning ...");
                let pubkey_package = flow_processor.frost_aggregator.run_dkg_flow(&request.musig_id).await
                    .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to run DKG flow: {}", e)))?;
                tracing::debug!("[{LOG_PATH}] DKG processing was successfully completed");
                pubkey_package
            }
            Some(x) => {
                tracing::debug!("[{LOG_PATH}] Musig exists, obtaining dkg pubkey ...");
                // extract data from db, get nonce and generate new one, return it to user
                match x.dkg_state {
                    AggregatorDkgState::DkgRound1 { .. } => {
                        return Err(FlowProcessorError::UnfinishedDkgState("Should be DkgFinalized, got DkgRound1".to_string()));
                    }
                    AggregatorDkgState::DkgRound2 { .. } => {
                        return Err(FlowProcessorError::UnfinishedDkgState("Should be DkgFinalized, got DkgRound2".to_string()));
                    }
                    AggregatorDkgState::DkgFinalized {
                        public_key_package: pubkey_package,
                    } => pubkey_package
                }
            }
        };


    let nonce = generate_nonce();
    let public_key = convert_public_key_package(&public_key_package)
        .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to convert public key package: {}", e)))?;
    let address = get_address(public_key, nonce, flow_processor.network)
        .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to create address: {}", e)))?;

    let verifiers_responses = VerifiersResponses::new(DepositStatus::Created, flow_processor.verifier_configs.iter().map(|v| v.id).collect());

    local_db_storage
        .set_deposit_addr_info(
            DepositAddrInfo {
                musig_id: request.musig_id.clone(),
                nonce,
                address: address.to_string(),
                is_btc: true,
                amount: request.amount,
                confirmation_status: verifiers_responses,
            },
        )
        .await?;

    Ok(address)
}
