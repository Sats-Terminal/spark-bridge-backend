use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::IssueSparkDepositAddressRequest;
use frost::traits::AggregatorMusigIdStorage;
use frost::types::AggregatorDkgState;
use frost::utils::convert_public_key_package;
use frost::utils::generate_nonce;
use gateway_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, DepositStatus, VerifiersResponses, InnerAddress,
};
use gateway_spark_service::utils::convert_network_to_spark_network;
use spark_address::{SparkAddressData, encode_spark_address};
use tracing;

pub async fn handle(
    flow_router: &mut FlowProcessorRouter,
    request: IssueSparkDepositAddressRequest,
) -> Result<String, FlowProcessorError> {
    let public_key_package = match flow_router.storage.get_musig_id_data(&request.musig_id).await? {
        None => {
            tracing::debug!("Missing musig, running dkg from the beginning ...");
            let pubkey_package = flow_router
                .frost_aggregator
                .run_dkg_flow(&request.musig_id)
                .await
                .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to run DKG flow: {}", e)))?;
            tracing::debug!("DKG processing was successfully completed");
            pubkey_package
        }
        Some(x) => {
            tracing::debug!("Musig exists, obtaining dkg pubkey ...");
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

    let nonce = generate_nonce();
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

    flow_router
        .storage
        .set_deposit_addr_info(DepositAddrInfo {
            musig_id: request.musig_id.clone(),
            nonce,
            deposit_address: InnerAddress::SparkAddress(address.clone()),
            bridge_address: None,
            is_btc: false,
            amount: request.amount,
            confirmation_status: verifiers_responses,
        })
        .await?;

    Ok(address)
}
