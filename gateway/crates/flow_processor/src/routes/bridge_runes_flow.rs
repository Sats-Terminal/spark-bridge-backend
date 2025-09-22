use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::BridgeRunesRequest;
use frost::traits::AggregatorMusigIdStorage;
use frost::types::MusigId;
use frost::utils::generate_issuer_public_key;
use gateway_local_db_store::schemas::deposit_address::DepositAddressStorage;
use gateway_spark_service::types::SparkTransactionType;
use gateway_spark_service::utils::{convert_network_to_spark_network, create_wrunes_metadata};
use std::str::FromStr;
use tracing::{info, instrument};

const LOG_PATH: &str = "flow_processor:routes:bridge_runes_flow";

#[instrument(skip(flow_processor), level = "trace", ret)]
pub async fn handle(
    flow_processor: &mut FlowProcessorRouter,
    request: BridgeRunesRequest,
) -> Result<(), FlowProcessorError> {
    info!("[{LOG_PATH}] Handling btc addr bridge runes flow ...");

    let response = flow_processor
        .storage
        .get_issuer_musig_id()
        .await
        .map_err(|e| FlowProcessorError::DbError(e))?;

    let issuer_musig_id = match response {
        Some(issuer_musig_id) => issuer_musig_id,
        None => {
            let issuer_public_key = generate_issuer_public_key();
            let rune_id = flow_processor
                .storage
                .get_row_by_deposit_address(request.btc_address.to_string())
                .await
                .map_err(|e| FlowProcessorError::DbError(e))?
                .map(|row| row.musig_id.get_rune_id())
                .ok_or(FlowProcessorError::InvalidDataError("Rune id not found".to_string()))?;
            let musig_id = MusigId::Issuer {
                issuer_public_key,
                rune_id: rune_id.clone(),
            };

            flow_processor
                .frost_aggregator
                .run_dkg_flow(&musig_id)
                .await
                .map_err(|e| {
                    FlowProcessorError::FrostAggregatorError(format!("Failed to run DKG flow for issuer: {}", e))
                })?;

            let wrunes_metadata = create_wrunes_metadata(rune_id);

            flow_processor
                .spark_service
                .send_spark_transaction(
                    musig_id.clone(),
                    None,
                    wrunes_metadata.token_identifier,
                    SparkTransactionType::Create {
                        token_name: wrunes_metadata.token_name,
                        token_ticker: wrunes_metadata.token_ticker,
                    },
                    convert_network_to_spark_network(flow_processor.network),
                )
                .await
                .map_err(|e| {
                    FlowProcessorError::SparkServiceError(format!("Failed to send spark create transaction: {}", e))
                })?;

            musig_id
        }
    };

    let wrunes_metadata = create_wrunes_metadata(issuer_musig_id.get_rune_id());

    let deposit_addr_info = flow_processor
        .storage
        .get_row_by_deposit_address(request.btc_address.to_string())
        .await
        .map_err(|e| FlowProcessorError::DbError(e))?
        .ok_or(FlowProcessorError::InvalidDataError(
            "Deposit address info not found".to_string(),
        ))?;

    let receiver_identity_public_key =
        bitcoin::secp256k1::PublicKey::from_str(&deposit_addr_info.bridge_address.ok_or(
            FlowProcessorError::InvalidDataError("Bridge address not found".to_string()),
        )?)
        .map_err(|e| {
            FlowProcessorError::InvalidDataError(format!("Failed to parse receiver identity public key: {}", e))
        })?;

    flow_processor
        .spark_service
        .send_spark_transaction(
            issuer_musig_id.clone(),
            None,
            wrunes_metadata.token_identifier,
            SparkTransactionType::Mint {
                receiver_identity_public_key,
                token_amount: deposit_addr_info.amount,
            },
            convert_network_to_spark_network(flow_processor.network),
        )
        .await
        .map_err(|e| FlowProcessorError::SparkServiceError(format!("Failed to send spark mint transaction: {}", e)))?;

    Ok(())
}
