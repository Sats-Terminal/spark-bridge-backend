use crate::error::DepositVerificationError;
use crate::traits::VerificationClient;
use crate::types::*;
use crate::types::{NotifyRunesDepositRequest, VerifyRunesDepositRequest, VerifySparkDepositRequest};
use bitcoin::Address;
use futures::future::join_all;
use gateway_flow_processor::flow_sender::{FlowSender, TypedMessageSender};
use gateway_flow_processor::types::{BridgeRunesRequest, ExitSparkRequest};
use gateway_local_db_store::schemas::deposit_address::{DepositAddressStorage, DepositStatus, VerifiersResponses, InnerAddress};
use gateway_local_db_store::schemas::utxo_storage::{Utxo, UtxoStatus, UtxoStorage};
use gateway_local_db_store::storage::LocalDbStorage;
use std::collections::HashMap;
use std::sync::Arc;
use tracing;
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct DepositVerificationAggregator {
    flow_sender: FlowSender,
    verifiers: HashMap<u16, Arc<dyn VerificationClient>>,
    storage: Arc<LocalDbStorage>,
}

impl DepositVerificationAggregator {
    pub fn new(
        flow_sender: FlowSender,
        verifiers: HashMap<u16, Arc<dyn VerificationClient>>,
        storage: Arc<LocalDbStorage>,
    ) -> Self {
        Self {
            flow_sender,
            verifiers,
            storage,
        }
    }

    pub async fn verify_runes_deposit(
        &self,
        request: VerifyRunesDepositRequest,
    ) -> Result<(), DepositVerificationError> {
        tracing::info!("Verifying runes deposit for address: {}", request.btc_address);

        self.storage
            .update_bridge_address_by_deposit_address(InnerAddress::BitcoinAddress(request.btc_address.clone()), InnerAddress::SparkAddress(request.bridge_address.clone()))
            .await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error updating bridge address: {:?}", e)))?;

        let deposit_addr_info = self
            .storage
            .get_row_by_deposit_address(InnerAddress::BitcoinAddress(request.btc_address.clone()))
            .await
            .map_err(|e| {
                DepositVerificationError::StorageError(format!("Error getting deposit address info: {:?}", e))
            })?
            .ok_or(DepositVerificationError::StorageError(
                "Deposit address info not found".to_string(),
            ))?;

        let watch_runes_deposit_request = WatchRunesDepositRequest {
            musig_id: deposit_addr_info.musig_id.clone(),
            nonce: deposit_addr_info.nonce,
            amount: deposit_addr_info.amount,
            btc_address: request.btc_address.clone(),
            bridge_address: request.bridge_address.clone(),
            out_point: request.out_point,
        };

        let mut futures = vec![];

        for (id, verifier) in self.verifiers.iter() {
            let watch_runes_deposit_request_clone = watch_runes_deposit_request.clone();
            let join_handle = async move {
                let response = verifier.watch_runes_deposit(watch_runes_deposit_request_clone).await;
                (id, response)
            };
            futures.push(join_handle);
        }

        let _ = join_all(futures)
            .await
            .into_iter()
            .map(|(_, result)| result)
            .collect::<Result<Vec<_>, DepositVerificationError>>()?;

        let ids = self.verifiers.keys().cloned().collect();
        let verifiers_responses = VerifiersResponses::new(DepositStatus::WaitingForConfirmation, ids);

        self.storage
            .set_confirmation_status_by_deposit_address(InnerAddress::BitcoinAddress(request.btc_address.clone()), verifiers_responses)
            .await
            .map_err(|e| {
                DepositVerificationError::StorageError(format!("Error updating confirmation status: {:?}", e))
            })?;

        let utxo = Utxo {
            out_point: request.out_point,
            btc_address: request.btc_address.clone(),
            rune_amount: deposit_addr_info.amount,
            rune_id: deposit_addr_info.musig_id.get_rune_id(),
            status: UtxoStatus::Pending,
            sats_fee_amount: 0,
        };
        self.storage
            .insert_utxo(utxo)
            .await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error inserting utxo: {:?}", e)))?;

        tracing::info!("Runes deposit verification sent for address: {}", request.btc_address.to_string());

        Ok(())
    }

    pub async fn notify_runes_deposit(
        &self,
        request: NotifyRunesDepositRequest,
    ) -> Result<(), DepositVerificationError> {
        tracing::info!(
            "Retrieving confirmation status for out_point: {}, verifier: {}",
            request.out_point,
            request.verifier_id
        );

        self.storage
            .update_sats_fee_amount(request.out_point, request.sats_fee_amount)
            .await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error updating sats fee amount: {:?}", e)))?;

        let utxo = self
            .storage
            .get_utxo(request.out_point)
            .await
            .map_err(|e| {
                DepositVerificationError::StorageError(format!("Error getting address by out point: {:?}", e))
            })?
            .ok_or(DepositVerificationError::StorageError("Address not found".to_string()))?;

        let btc_address = utxo.btc_address;

        self.storage
            .update_confirmation_status_by_deposit_address(InnerAddress::BitcoinAddress(btc_address.clone()), request.verifier_id, request.status)
            .await
            .map_err(|e| {
                DepositVerificationError::StorageError(format!("Error updating confirmation status: {:?}", e))
            })?;

        let confirmation_status_info = self
            .storage
            .get_row_by_deposit_address(InnerAddress::BitcoinAddress(btc_address.clone()))
            .await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error getting confirmation status: {:?}", e)))?
            .ok_or(DepositVerificationError::StorageError(
                "Confirmation status not found".to_string(),
            ))?
            .confirmation_status;

        let all_verifiers_confirmed = confirmation_status_info.check_all_verifiers_confirmed();

        if all_verifiers_confirmed {
            self.storage
                .update_status(request.out_point, UtxoStatus::Confirmed)
                .await
                .map_err(|e| DepositVerificationError::StorageError(format!("Error updating utxo status: {:?}", e)))?;

            self.flow_sender
                .send(BridgeRunesRequest {
                    btc_address: btc_address.clone(),
                })
                .await
                .map_err(|e| {
                    DepositVerificationError::FlowProcessorError(format!("Error sending bridge runes request: {:?}", e))
                })?;
            tracing::info!("Bridge runes request sent for address");
        }

        tracing::info!(
            "Runes deposit verification completed for verifier: {}, address: {}",
            request.verifier_id,
            btc_address
        );

        Ok(())
    }

    pub async fn verify_spark_deposit(
        &self,
        request: VerifySparkDepositRequest,
    ) -> Result<(), DepositVerificationError> {
        self.storage
            .update_bridge_address_by_deposit_address(InnerAddress::SparkAddress(request.spark_address.clone()), InnerAddress::BitcoinAddress(request.exit_address.clone()))
            .await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error updating bridge address: {:?}", e)))?;

        let deposit_addr_info = self
            .storage
            .get_row_by_deposit_address(InnerAddress::SparkAddress(request.spark_address.clone()))
            .await
            .map_err(|e| {
                DepositVerificationError::StorageError(format!("Error getting deposit address info: {:?}", e))
            })?
            .ok_or(DepositVerificationError::StorageError(
                "Deposit address info not found".to_string(),
            ))?;

        let watch_spark_deposit_request = WatchSparkDepositRequest {
            musig_id: deposit_addr_info.musig_id.clone(),
            nonce: deposit_addr_info.nonce,
            spark_address: request.spark_address.clone(),
            amount: deposit_addr_info.amount,
            exit_address: request.exit_address.clone(),
        };

        let mut futures = vec![];

        for (id, verifier) in self.verifiers.iter() {
            let watch_spark_deposit_request_clone = watch_spark_deposit_request.clone();
            let join_handle = async move {
                let response = verifier.watch_spark_deposit(watch_spark_deposit_request_clone).await;
                (id, response)
            };
            futures.push(join_handle);
        }

        let responses = join_all(futures)
            .await
            .into_iter()
            .map(|(id, result)| result.map(|response| (*id, response)))
            .collect::<Result<Vec<(u16, WatchSparkDepositResponse)>, DepositVerificationError>>()?;

        let ids = responses.iter().map(|(id, _)| *id).collect();
        let mut verifiers_responses = VerifiersResponses::new(DepositStatus::Created, ids);
        for (id, response) in responses {
            verifiers_responses.responses.insert(id, response.verifier_response);
        }

        let all_verifiers_confirmed = verifiers_responses.check_all_verifiers_confirmed();

        self.storage
            .set_confirmation_status_by_deposit_address(InnerAddress::SparkAddress(request.spark_address.clone()), verifiers_responses)
            .await
            .map_err(|e| {
                DepositVerificationError::StorageError(format!("Error updating confirmation status: {:?}", e))
            })?;

        if all_verifiers_confirmed {
            self.flow_sender
                .send(ExitSparkRequest {
                    spark_address: request.spark_address.clone(),
                })
                .await
                .map_err(|e| {
                    DepositVerificationError::FlowProcessorError(format!("Error sending bridge spark request: {:?}", e))
                })?;
        }

        Ok(())
    }
}
