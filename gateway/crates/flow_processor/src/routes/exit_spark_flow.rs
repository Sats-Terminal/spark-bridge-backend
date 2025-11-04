use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::ExitSparkRequest;
use bitcoin::OutPoint;
use bitcoin::secp256k1::schnorr::Signature;
use frost::types::SigningMetadata;
use frost::utils::{convert_public_key_package, generate_tweak_bytes, get_tweaked_p2tr_address};
use gateway_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, InnerAddress, VerifiersResponses,
};
use gateway_local_db_store::schemas::paying_utxo::PayingUtxoStorage;
use gateway_local_db_store::schemas::utxo_storage::{Utxo, UtxoStatus, UtxoStorage};
use gateway_rune_transfer::transfer::RuneTransferOutput;
use gateway_rune_transfer::transfer::{
    add_signature_to_transaction, create_message_hash, create_rune_partial_transaction,
};
use persistent_storage::error::DbError;
use tracing::instrument;

const DUST_AMOUNT: u64 = 546;

#[instrument(level = "trace", skip(flow_router), ret)]
pub async fn handle(
    flow_router: &mut FlowProcessorRouter,
    request: ExitSparkRequest,
) -> Result<(), FlowProcessorError> {
    tracing::info!("Handling exit spark flow ...");

    let deposit_addr_info = flow_router
        .storage
        .get_row_by_deposit_address(InnerAddress::SparkAddress(request.spark_address.clone()))
        .await?
        .ok_or(FlowProcessorError::DbError(DbError::NotFound(
            "Deposit address info not found".to_string(),
        )))?;

    let exit_address = match deposit_addr_info.bridge_address {
        Some(address) => &address.to_bitcoin_address()?,
        None => {
            return Err(FlowProcessorError::InvalidDataError(
                "Bridge address not found".to_string(),
            ));
        }
    };

    let paying_utxo = flow_router
        .storage
        .get_paying_utxo_by_btc_exit_address(exit_address.clone())
        .await?
        .ok_or(FlowProcessorError::DbError(DbError::NotFound(
            "Paying UTXO not found".to_string(),
        )))?;

    let utxos = flow_router
        .storage
        .select_utxos_for_amount(deposit_addr_info.musig_id.get_rune_id(), deposit_addr_info.amount)
        .await?;
    let total_amount = utxos.iter().map(|utxo| utxo.rune_amount).sum::<u64>();
    let exit_amount = deposit_addr_info.amount;

    let outputs_to_spend = utxos.iter().map(|utxo| utxo.out_point).collect::<Vec<OutPoint>>();

    let mut rune_transfer_outputs = vec![RuneTransferOutput {
        address: exit_address.clone(),
        sats_amount: DUST_AMOUNT,
        runes_amount: exit_amount,
    }];

    if total_amount > exit_amount {
        let new_nonce = generate_tweak_bytes();
        let public_key_package = flow_router
            .frost_aggregator
            .get_public_key_package(deposit_addr_info.musig_id.clone(), Some(new_nonce))
            .await
            .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to get public key package: {e}")))?;
        let public_key = convert_public_key_package(&public_key_package)
            .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to convert public key package: {e}")))?;
        let deposit_address = get_tweaked_p2tr_address(public_key, new_nonce, flow_router.network)
            .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to create address: {e}")))?;

        flow_router
            .storage
            .set_deposit_addr_info(DepositAddrInfo {
                musig_id: deposit_addr_info.musig_id.clone(),
                nonce: new_nonce,
                deposit_address: InnerAddress::BitcoinAddress(deposit_address.clone()),
                bridge_address: None,
                is_btc: true,
                amount: total_amount - exit_amount,
                confirmation_status: VerifiersResponses::empty(),
            })
            .await?;

        rune_transfer_outputs.push(RuneTransferOutput {
            address: deposit_address,
            sats_amount: DUST_AMOUNT,
            runes_amount: total_amount - exit_amount,
        });
    }

    tracing::info!("Creating rune partial transaction");
    let mut transaction = create_rune_partial_transaction(
        outputs_to_spend,
        paying_utxo,
        rune_transfer_outputs.clone(),
        deposit_addr_info.musig_id.get_rune_id(),
    )
    .map_err(|e| FlowProcessorError::RuneTransferError(format!("Failed to create rune partial transaction: {e}")))?;

    for i in 1..transaction.input.len() {
        let input_btc_address = utxos[i - 1].btc_address.clone();
        let message_hash =
            create_message_hash(&transaction, input_btc_address.clone(), utxos[i - 1].sats_fee_amount, i)
                .map_err(|e| FlowProcessorError::RuneTransferError(format!("Failed to create message hash: {e}")))?;

        let input_deposit_addr_info = flow_router
            .storage
            .get_row_by_deposit_address(InnerAddress::BitcoinAddress(input_btc_address.clone()))
            .await?
            .ok_or(FlowProcessorError::DbError(DbError::NotFound(
                "Input deposit address info not found".to_string(),
            )))?;

        let signature_bytes = flow_router
            .frost_aggregator
            .run_signing_flow(
                input_deposit_addr_info.musig_id,
                message_hash.as_ref(),
                SigningMetadata::BtcTransactionMetadata {},
                Some(input_deposit_addr_info.nonce),
                true,
            )
            .await
            .map_err(|e| FlowProcessorError::FrostAggregatorError(format!("Failed to sign message hash: {e}")))?
            .serialize()
            .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to serialize signature: {e}")))?;

        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|e| FlowProcessorError::InvalidDataError(format!("Failed to deserialize signature: {e}")))?;

        add_signature_to_transaction(&mut transaction, i, signature);
    }

    flow_router
        .bitcoin_client
        .broadcast_transaction(transaction.clone())
        .await
        .map_err(|e| FlowProcessorError::RuneTransferError(format!("Failed to broadcast transaction: {e}")))?;

    if total_amount > exit_amount {
        let utxo = Utxo {
            out_point: OutPoint::new(transaction.compute_txid(), 1), // Change utxo
            btc_address: rune_transfer_outputs[1].address.clone(),   // Change utxo address
            rune_amount: total_amount - exit_amount,
            rune_id: deposit_addr_info.musig_id.get_rune_id(),
            status: UtxoStatus::Confirmed,
            sats_fee_amount: transaction.output[1].value.to_sat(),
        };

        flow_router.storage.insert_utxo(utxo).await?;
    }

    tracing::info!("Exit spark flow completed");

    Ok(())
}
