use crate::error::FlowProcessorError;
use bitcoin::Network;
use frost::aggregator::FrostAggregator;
use frost::traits::AggregatorMusigIdStorage;
use frost::types::AggregatorDkgState;
use frost::types::MusigId;
use frost::utils::convert_public_key_package;
use frost::utils::generate_nonce;
use gateway_config_parser::config::VerifierConfig;
use gateway_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, DepositStatus, DepositStatusInfo, VerifiersResponses,
};
use gateway_local_db_store::storage::LocalDbStorage;
use spark_client::utils::spark_address::{SparkAddressData, encode_spark_address};
use std::sync::Arc;
use tracing;

pub async fn handle(
    verifier_configs: Arc<Vec<VerifierConfig>>,
    musig_id: MusigId,
    amount: u64,
    network: Network,
    frost_aggregator: FrostAggregator,
    storage: Arc<LocalDbStorage>,
) -> Result<String, FlowProcessorError> {
    let public_key_package = match storage.get_musig_id_data(&musig_id).await? {
        None => {
            tracing::debug!("Missing musig, running dkg from the beginning ...");
            let pubkey_package = frost_aggregator
                .run_dkg_flow(&musig_id)
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

    let address = encode_spark_address(&SparkAddressData {
        identity_public_key: public_key.to_string(),
        network: network.into(),
    })
    .map_err(|e| FlowProcessorError::InvalidDataError(e.to_string()))?;

    let verifiers_responses =
        VerifiersResponses::new(DepositStatus::Created, verifier_configs.iter().map(|v| v.id).collect());

    storage
        .set_deposit_addr_info(
            &musig_id,
            nonce,
            DepositAddrInfo {
                address: Some(address.clone()),
                is_btc: true,
                amount,
                txid: None,
                confirmation_status: DepositStatusInfo {
                    status: DepositStatus::Created,
                    verifiers_responses,
                },
            },
        )
        .await?;

    Ok(address)
}
