use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use tracing::{info, instrument};
use crate::types::ExitSparkRequest;
use gateway_local_db_store::schemas::utxo_storage::UtxoStorage;
use gateway_local_db_store::schemas::deposit_address::{DepositAddressStorage, DepositAddrInfo, DepositStatus, VerifiersResponses};
use gateway_local_db_store::schemas::paying_utxo::PayingUtxoStorage;
use persistent_storage::error::DbError;
use gateway_rune_transfer::transfer::{create_rune_partial_transaction, create_message_hash, add_signature_to_transaction};
use gateway_rune_transfer::transfer::RuneTransferOutput;
use frost::utils::{generate_nonce, get_address, convert_public_key_package};
use bitcoin::OutPoint;
use bitcoin::secp256k1::schnorr::Signature;
use global_utils::conversion::decode_address;
use frost::types::SigningMetadata;

const DUST_AMOUNT: u64 = 546;

const LOG_PATH: &str = "flow_processor:routes:exit_spark_flow";

#[instrument(level = "info", skip(flow_router), ret)]
pub async fn handle(
    flow_router: &mut FlowProcessorRouter,
    request: ExitSparkRequest,
) -> Result<(), FlowProcessorError> {
    info!("[{LOG_PATH}] Handling exit spark flow ...");

    let deposit_addr_info = flow_router.storage.get_row_by_deposit_address(request.spark_address.clone()).await?
        .ok_or(FlowProcessorError::DbError(DbError::NotFound("Deposit address info not found".to_string())))?;

    let exit_address = match deposit_addr_info.bridge_address {
        Some(address) => decode_address(&address, flow_router.network)
            .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to parse exit address: {e}")))?,
        None => {
            return Err(FlowProcessorError::InvalidDataError("Bridge address not found".to_string()));
        }
    };

    let paying_utxo = flow_router.storage.get_paying_utxo_by_spark_deposit_address(request.spark_address).await?
        .ok_or(FlowProcessorError::DbError(DbError::NotFound("Paying UTXO not found".to_string())))?;

    let utxos = flow_router.storage.select_utxos_for_amount(deposit_addr_info.musig_id.get_rune_id(), deposit_addr_info.amount).await?;
    let total_amount = utxos.iter().map(|utxo| utxo.rune_amount).sum::<u64>();
    let exit_amount = deposit_addr_info.amount;

    let user_utxo = flow_router.storage.get_utxo_by_btc_address(exit_address.to_string()).await?
        .ok_or(FlowProcessorError::DbError(DbError::NotFound("User UTXO not found".to_string())))?;

    let outputs_to_spend = utxos.iter().map(|utxo| utxo.out_point).collect::<Vec<OutPoint>>();
    
    let mut rune_transfer_outputs = vec![
        RuneTransferOutput {
            address: exit_address.clone().to_string(),
            sats_amount: DUST_AMOUNT,
            runes_amount: exit_amount,
        },
    ];

    if total_amount > exit_amount {
        let new_nonce = generate_nonce();
        let public_key_package = flow_router.frost_aggregator.get_public_key_package(deposit_addr_info.musig_id.clone(), Some(new_nonce)).await
            .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to get public key package: {e}")))?;
        let public_key = convert_public_key_package(&public_key_package)
            .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to convert public key package: {e}")))?;
        let deposit_address = get_address(public_key, new_nonce, flow_router.network)
            .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to create address: {e}")))?;

        flow_router.storage.set_deposit_addr_info(DepositAddrInfo {
            musig_id: deposit_addr_info.musig_id.clone(),
            nonce: new_nonce,
            deposit_address: deposit_address.to_string(),
            bridge_address: None,
            is_btc: true,
            amount: total_amount - exit_amount,
            confirmation_status: VerifiersResponses::empty(),
        })
        .await?;

        rune_transfer_outputs.push(RuneTransferOutput {
            address: deposit_address.to_string(),
            sats_amount: DUST_AMOUNT,
            runes_amount: total_amount - exit_amount,
        });
    }

    let mut transaction = create_rune_partial_transaction(
        outputs_to_spend,
        paying_utxo,
        rune_transfer_outputs,
        deposit_addr_info.musig_id.get_rune_id(),
        flow_router.network,
    )
        .map_err(|e| FlowProcessorError::RuneTransferError(format!("Failed to create rune partial transaction: {e}")))?;

    for i in 0..(transaction.input.len() - 1) { // -1 because the last input is the paying input
        let message_hash = create_message_hash(&transaction, exit_address.clone(), DUST_AMOUNT, i)
            .map_err(|e| FlowProcessorError::RuneTransferError(format!("Failed to create message hash: {e}")))?;

        let input_btc_address = utxos[i].btc_address.clone();
        let input_deposit_addr_info = flow_router.storage.get_row_by_deposit_address(input_btc_address).await?
            .ok_or(FlowProcessorError::DbError(DbError::NotFound("Input deposit address info not found".to_string())))?;

        let signature_bytes = flow_router.frost_aggregator.run_signing_flow(
            input_deposit_addr_info.musig_id,
            message_hash.as_ref(),
            SigningMetadata::BtcTransactionMetadata {},
            Some(input_deposit_addr_info.nonce),
        )
            .await
            .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to sign message hash: {e}")))?
            .serialize()
            .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to serialize signature: {e}")))?;

        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to deserialize signature: {e}")))?;

        add_signature_to_transaction(&mut transaction, i, signature);
    }
    

    Ok(())
}
