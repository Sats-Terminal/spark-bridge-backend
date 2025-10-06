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
use gateway_local_db_store::schemas::user_identifier::{UserIdentifierData, UserIdentifierStorage, UserIds};
use global_utils::conversion::convert_network_to_spark_network;
use spark_address::{SparkAddressData, encode_spark_address};
use tracing::instrument;

#[instrument(skip(flow_router), level = "trace", ret)]
pub async fn handle(
    flow_router: &mut FlowProcessorRouter,
    request: IssueSparkDepositAddressRequest,
) -> Result<String, FlowProcessorError> {
    tracing::info!("Handling spark addr issuing for musig id: {:?}", request.musig_id);
    let local_db_storage = flow_router.storage.clone();

    let (public_key_package, user_uuid, rune_id) = match local_db_storage.get_ids_by_musig_id(&request.musig_id).await?
    {
        None => {
            tracing::debug!("Missing DkgShareId, running dkg from the beginning ...");

            let user_identifier_data = UserIdentifierData {
                public_key: request.musig_id.get_public_key().to_string(),
                rune_id: request.musig_id.get_rune_id(),
                is_issuer: false,
            };
            let user_ids = local_db_storage
                .get_random_unused_dkg_share(user_identifier_data.clone())
                .await
                .map_err(|e| FlowProcessorError::FailedToObtainRandomDkgShare {
                    user_identifier_data,
                    err: e.to_string(),
                })?;

            // let pubkey_package = flow_router
            //     .frost_aggregator
            //     .run_dkg_flow(&user_ids.dkg_share_id)
            //     .await
            //     .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to run DKG flow: {}", e)))?;

            //todo: handle error
            let pubkey_package = flow_router
                .frost_aggregator
                .get_public_key_package(user_ids.dkg_share_id, None)
                .await
                .unwrap();
            tracing::debug!("DKG processing was successfully completed");
            (pubkey_package, user_ids.user_uuid, user_ids.rune_id)
        }
        Some(ids) => {
            tracing::debug!("DkgShare exists, obtaining dkg pubkey ...");
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

    let nonce = generate_tweak_bytes();
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
        .set_deposit_addr_info(DepositAddrInfo {
            user_uuid,
            rune_id,
            nonce,
            deposit_address: InnerAddress::SparkAddress(address.clone()),
            bridge_address: None,
            is_btc: false,
            amount: request.amount,
            confirmation_status: verifiers_responses,
        })
        .await?;

    tracing::info!("Spark addr issuing completed for musig id: {:?}", request.musig_id);

    Ok(address)
}
