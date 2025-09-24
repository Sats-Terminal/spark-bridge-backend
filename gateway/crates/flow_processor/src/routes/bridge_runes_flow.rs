use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::BridgeRunesRequest;
use frost::traits::AggregatorMusigIdStorage;
use frost::types::MusigId;
use frost::utils::generate_issuer_public_key;
use gateway_local_db_store::schemas::deposit_address::{DepositAddressStorage, InnerAddress};
use gateway_spark_service::types::SparkTransactionType;
use gateway_spark_service::utils::{convert_network_to_spark_network, create_wrunes_metadata};
use tracing::{info, instrument};
use spark_address::{encode_spark_address, SparkAddressData};

const LOG_PATH: &str = "flow_processor:routes:bridge_runes_flow";

#[instrument(skip(flow_processor), level = "trace", ret)]
pub async fn handle(
    flow_processor: &mut FlowProcessorRouter,
    request: BridgeRunesRequest,
) -> Result<(), FlowProcessorError> {
    info!("[{LOG_PATH}] Handling btc addr bridge runes flow ...");

    let deposit_addr_info = flow_processor
        .storage
        .get_row_by_deposit_address(InnerAddress::BitcoinAddress(request.btc_address.clone()))
        .await
        .map_err(FlowProcessorError::DbError)?
        .ok_or(FlowProcessorError::InvalidDataError("Deposit address info not found".to_string()))?;

    let rune_id = deposit_addr_info.musig_id.get_rune_id();

    let issuer_musig_id = flow_processor
        .storage
        .get_issuer_musig_id(rune_id.clone())
        .await
        .map_err(FlowProcessorError::DbError)?;

    let issuer_musig_id = match issuer_musig_id {
        Some(issuer_musig_id) => issuer_musig_id,
        None => {
            let issuer_public_key = generate_issuer_public_key();

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

            let wrunes_metadata = create_wrunes_metadata(rune_id.clone());

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

    let wrunes_metadata = create_wrunes_metadata(rune_id.clone());

    let deposit_addr_info = flow_processor
        .storage
        .get_row_by_deposit_address(InnerAddress::BitcoinAddress(request.btc_address.clone()))
        .await
        .map_err(FlowProcessorError::DbError)?
        .ok_or(FlowProcessorError::InvalidDataError(
            "Deposit address info not found".to_string(),
        ))?;

        // TODO: remove this once we have deposit verification flow
    let receiver_spark_address = encode_spark_address(SparkAddressData {
        identity_public_key: deposit_addr_info.musig_id.get_public_key().to_string(),
        network: convert_network_to_spark_network(flow_processor.network),
        invoice: None,
        signature: None,
    })
        .map_err(|e| FlowProcessorError::SparkAddressError(e))?;

    flow_processor
        .spark_service
        .send_spark_transaction(
            issuer_musig_id.clone(),
            None,
            wrunes_metadata.token_identifier,
            SparkTransactionType::Mint {
                receiver_spark_address,
                token_amount: deposit_addr_info.amount,
            },
            convert_network_to_spark_network(flow_processor.network),
        )
        .await
        .map_err(|e| FlowProcessorError::SparkServiceError(format!("Failed to send spark mint transaction: {}", e)))?;

    Ok(())
}
