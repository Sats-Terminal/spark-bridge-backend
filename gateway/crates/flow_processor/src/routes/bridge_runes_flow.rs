use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::BridgeRunesRequest;
use frost::traits::AggregatorDkgShareStorage;
use frost::utils::generate_issuer_public_key;
use gateway_local_db_store::schemas::deposit_address::{DepositAddressStorage, InnerAddress};
use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
use gateway_local_db_store::schemas::user_identifier::{UserIdentifier, UserIdentifierStorage, UserIds};
use gateway_spark_service::types::SparkTransactionType;
use gateway_spark_service::utils::{convert_network_to_spark_network, create_wrunes_metadata};
use global_utils::common_types::get_uuid;
use spark_address::{SparkAddressData, encode_spark_address};
use tracing::{info, instrument};

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
        .ok_or(FlowProcessorError::InvalidDataError(
            "Deposit address info not found".to_string(),
        ))?;
    let user_info = flow_processor
        .storage
        .get_row_by_user_uuid(&deposit_addr_info.user_uuid)
        .await
        .map_err(FlowProcessorError::DbError)?
        .ok_or(FlowProcessorError::InvalidDataError("User info not found".to_string()))?;
    let rune_id = user_info.rune_id;

    let possible_issuer_user_ids = flow_processor
        .storage
        .get_issuer_ids(rune_id.clone())
        .await
        .map_err(FlowProcessorError::DbError)?;

    let issuer_user_ids = match possible_issuer_user_ids {
        Some(user_ids) => user_ids,
        None => {
            let issuer_public_key = generate_issuer_public_key();

            let dkg_share_id = flow_processor.storage.get_random_unused_dkg_share().await?;
            let user_uuid = get_uuid();
            let user_identifier = UserIdentifier {
                user_uuid,
                dkg_share_id,
                rune_id: rune_id.clone(),
                public_key: issuer_public_key.to_string(),
                is_issuer: true,
            };

            // Dkg flow has to be already completed

            let wrunes_metadata = create_wrunes_metadata(rune_id.clone());

            flow_processor
                .spark_service
                .send_spark_transaction(
                    user_identifier.clone(),
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

            UserIds {
                user_uuid,
                dkg_share_id,
            }
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
    let user_identifier = flow_processor
        .storage
        .get_row_by_user_uuid(&deposit_addr_info.user_uuid)
        .await
        .map_err(FlowProcessorError::DbError)?
        .ok_or(FlowProcessorError::InvalidDataError("User info not found".to_string()))?;

    // TODO: remove this once we have deposit verification flow
    let receiver_spark_address = encode_spark_address(SparkAddressData {
        identity_public_key: user_identifier.public_key,
        network: convert_network_to_spark_network(flow_processor.network),
        invoice: None,
        signature: None,
    })
    .map_err(|e| FlowProcessorError::SparkAddressError(e))?;

    flow_processor
        .spark_service
        .send_spark_transaction(
            issuer_user_ids.clone(),
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
