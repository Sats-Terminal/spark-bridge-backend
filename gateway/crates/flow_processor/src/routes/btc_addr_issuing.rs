use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::IssueBtcDepositAddressRequest;
use bitcoin::Address;
use frost::traits::AggregatorDkgShareStorage;
use frost::types::{AggregatorDkgShareData, AggregatorDkgState};
use frost::utils::convert_public_key_package;
use frost::utils::{generate_tweak_bytes, get_tweaked_p2tr_address};
use gateway_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, DepositStatus, InnerAddress, VerifiersResponses,
};
use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
use gateway_local_db_store::schemas::user_identifier::{
    UserIdentifier, UserIdentifierData, UserIdentifierStorage, UserIds,
};
use tracing;

const LOG_PATH: &str = "flow_processor:routes:btc_addr_issuing";

pub async fn handle(
    flow_processor: &mut FlowProcessorRouter,
    request: IssueBtcDepositAddressRequest,
) -> Result<Address, FlowProcessorError> {
    let local_db_storage = flow_processor.storage.clone();

    let (public_key_package, user_uuid, rune_id) = match local_db_storage.get_ids_by_musig_id(&request.musig_id).await?
    {
        None => {
            tracing::debug!("[{LOG_PATH}] Missing DkgShareId, running dkg from the beginning ...");
            let user_ids = local_db_storage
                .get_random_unused_dkg_share(UserIdentifierData {
                    public_key: request.musig_id.get_public_key().to_string(),
                    rune_id: request.musig_id.get_rune_id(),
                    is_issuer: false,
                })
                .await?;

            let pubkey_package = flow_processor
                .frost_aggregator
                .run_dkg_flow(&user_ids.dkg_share_id)
                .await
                .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to run DKG flow: {}", e)))?;
            tracing::debug!("[{LOG_PATH}] DKG processing was successfully completed");
            (pubkey_package, user_ids.user_uuid, user_ids.rune_id)
        }
        Some(ids) => {
            tracing::debug!("DkgShareId exists, obtaining dkg pubkey ...");
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
                Some(AggregatorDkgShareData { dkg_state }) => {
                    tracing::debug!("[{LOG_PATH}] Musig exists, obtaining dkg pubkey ...");
                    match dkg_state {
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
                    }
                }
            }
        }
    };

    let nonce = generate_tweak_bytes();
    let public_key = convert_public_key_package(&public_key_package)
        .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to convert public key package: {}", e)))?;
    let address = get_tweaked_p2tr_address(public_key, nonce, flow_processor.network)
        .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to create address: {}", e)))?;

    let verifiers_responses = VerifiersResponses::new(
        DepositStatus::Created,
        flow_processor.verifier_configs.iter().map(|v| v.id).collect(),
    );

    local_db_storage
        .set_deposit_addr_info(DepositAddrInfo {
            user_uuid,
            rune_id,
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
