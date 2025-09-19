use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use tracing::{info, instrument};
use crate::types::ExitSparkRequest;
use gateway_local_db_store::schemas::utxo_storage::UtxoStorage;
use gateway_local_db_store::schemas::deposit_address::DepositAddressStorage;
use frost::traits::AggregatorMusigIdStorage;
use persistent_storage::error::DbError;
use gateway_rune_transfer::transfer::{create_rune_transfer, create_message_hash};
use bitcoin::Address;
use std::str::FromStr;

const DUST_AMOUNT: u64 = 546;

const LOG_PATH: &str = "flow_processor:routes:exit_spark_flow";

#[instrument(level = "info", skip(flow_router), ret)]
pub async fn handle(
    flow_router: &mut FlowProcessorRouter,
    request: ExitSparkRequest,
) -> Result<(), FlowProcessorError> {
    info!("[{LOG_PATH}] Handling exit spark flow ...");

    let deposit_addr_info = flow_router.storage.get_row_by_deposit_address(request.spark_address).await?
        .ok_or(FlowProcessorError::DbError(DbError::NotFound("Deposit address info not found".to_string())))?;

    let exit_address = match deposit_addr_info.bridge_address {
        Some(address) => {
            Address::from_str(&address).map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to parse exit address: {e}")))?
            .require_network(flow_router.network)
            .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to parse exit address: {e}")))?
        }
        None => {
            return Err(FlowProcessorError::InvalidDataError("Bridge address not found".to_string()));
        }
    };

    let utxos = flow_router.storage.select_utxos_for_amount(deposit_addr_info.musig_id.get_rune_id(), deposit_addr_info.amount).await?;
    let total_amount = utxos.iter().map(|utxo| utxo.rune_amount).sum::<u64>();
    let exit_amount = deposit_addr_info.amount;

    let user_utxo = flow_router.storage.get_utxo_by_btc_address(exit_address.to_string()).await?
        .ok_or(FlowProcessorError::DbError(DbError::NotFound("User UTXO not found".to_string())))?;

    let outputs_to_spend = utxos.iter().map(|utxo| utxo.out_point).collect::<Vec<OutPoint>>();
    let output_addresses = vec![
        exit_address, 
        Address::from_str(&user_utxo.btc_address).map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to parse user utxo address: {e}")))?
            .require_network(flow_router.network)
            .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to parse user utxo address: {e}")))?
    ];
    let output_sats_amounts = vec![
        DUST_AMOUNT,
        DUST_AMOUNT,
    ];
    let output_runes_amounts = vec![
        exit_amount,
        total_amount - exit_amount,
    ];

    let transaction = create_rune_transfer(
        outputs_to_spend,
        output_addresses,
        output_sats_amounts,
        output_runes_amounts,
        deposit_addr_info.musig_id.get_rune_id(),
    ).map_err(|e| FlowProcessorError::RuneTransferError(format!("Failed to create rune transfer: {e}")))?;

    // TODO: finish exit flow

    Ok(())
}
