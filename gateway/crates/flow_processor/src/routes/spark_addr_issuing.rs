use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::IssueSparkDepositAddressRequest;
use frost::traits::AggregatorDkgShareStorage;
use frost::types::{AggregatorDkgShareData, AggregatorDkgState, DkgShareId};
use frost::utils::convert_public_key_package;
use frost::utils::generate_nonce;
use gateway_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, DepositStatus, InnerAddress, VerifiersResponses,
};
use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
use gateway_local_db_store::schemas::user_identifier::{UserIdentifierData, UserIdentifierStorage, UserIds};
use gateway_spark_service::utils::convert_network_to_spark_network;
use global_utils::common_types::get_uuid;
use spark_address::{SparkAddressData, encode_spark_address};
use tracing;

pub async fn handle(
    flow_processor: &mut FlowProcessorRouter,
    request: IssueSparkDepositAddressRequest,
) -> Result<String, FlowProcessorError> {
    let local_db_storage = flow_processor.storage.clone();

    let (public_key_package, user_uuid, rune_id) = match local_db_storage.get_ids_by_musig_id(&request.musig_id).await?
    {
        None => {
            tracing::debug!("Missing DkgShareId, running dkg from the beginning ...");

            let dkg_share_id: DkgShareId = local_db_storage.get_random_unused_dkg_share().await?;

            // Assign to user some uuid | Add to `gateway.user_identifier` table | but we don't return this value, waiting for next invocation
            let user_uuid = get_uuid();
            let rune_id = request.musig_id.get_rune_id();
            let _ = local_db_storage.set_user_identifier_data(
                &user_uuid,
                &dkg_share_id,
                UserIdentifierData {
                    public_key: request.musig_id.get_public_key().to_string(),
                    rune_id: rune_id.clone(),
                    is_issuer: false,
                },
            );

            let pubkey_package = flow_processor
                .frost_aggregator
                .run_dkg_flow(&dkg_share_id)
                .await
                .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to run DKG flow: {}", e)))?;
            tracing::debug!("DKG processing was successfully completed");
            (pubkey_package, user_uuid, rune_id)
        }
        Some(ids) => {
            tracing::debug!("Musig exists, obtaining dkg pubkey ...");
            // extract data from db, get nonce and generate new one, return it to user

            let UserIds {
                user_uuid,
                dkg_share_id,
                rune_id,
            } = ids;
            match local_db_storage.get_dkg_share_agg_data(&dkg_share_id).await? {
                None => {
                    return Err(FlowProcessorError::UnfinishedDkgState(
                        "Should be DkgFinalized, got None".to_string(),
                    ));
                }
                Some(AggregatorDkgShareData { dkg_state }) => match dkg_state {
                    AggregatorDkgState::Initialized => {
                        return Err(FlowProcessorError::UnfinishedDkgState(
                            "Should be DkgFinalized, got Initialized".to_string(),
                        ));
                    }
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
                    } => (pubkey_package, user_uuid, rune_id),
                },
            }
        }
    };

    let nonce = generate_nonce();
    let public_key = convert_public_key_package(&public_key_package)
        .map_err(|e| FlowProcessorError::InvalidDataError(e.to_string()))?;

    let address = encode_spark_address(SparkAddressData {
        identity_public_key: public_key.to_string(),
        network: convert_network_to_spark_network(flow_processor.network),
        invoice: None,
        signature: None,
    })
    .map_err(|e| FlowProcessorError::InvalidDataError(e.to_string()))?;
    let verifiers_responses = VerifiersResponses::new(
        DepositStatus::Created,
        flow_processor.verifier_configs.iter().map(|v| v.id).collect(),
    );

    local_db_storage
        .set_deposit_addr_info(DepositAddrInfo {
            user_uuid,
            rune_id,
            nonce,
            deposit_address: InnerAddress::SparkAddress(address.clone()),
            bridge_address: None,
            is_btc: true,
            amount: request.amount,
            confirmation_status: verifiers_responses,
        })
        .await?;

    Ok(address)
}
