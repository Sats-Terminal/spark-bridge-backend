use gateway_flow_processor::flow_sender::{FlowSender, TypedMessageSender};
use crate::traits::VerificationClient;
use std::sync::Arc;
use crate::types::*;
use crate::error::DepositVerificationError;
use gateway_local_db_store::schemas::deposit_address::{DepositAddressStorage, DepositStatusInfo, DepositStatus, VerifiersResponses};
use bitcoin::{Address, Txid};
use futures::future::join_all;
use gateway_flow_processor::types::{BridgeRunesRequest, ExitSparkRequest};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct DepositVerificationAggregator {
    flow_sender: FlowSender,
    verifiers: HashMap<u16, Arc<dyn VerificationClient>>,
    storage: Arc<dyn DepositAddressStorage>,
}

impl DepositVerificationAggregator {
    pub fn new(
        flow_sender: FlowSender,
        verifiers: HashMap<u16, Arc<dyn VerificationClient>>,
        storage: Arc<dyn DepositAddressStorage>,
    ) -> Self {
        Self { flow_sender, verifiers, storage }
    }

    pub async fn verify_runes_deposit(
        &self,
        btc_address: Address,
        txid: Txid,
    ) -> Result<(), DepositVerificationError> {
        let (musig_id, nonce, deposit_addr_info) = self.storage.get_row_by_address(btc_address.to_string()).await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error getting deposit address info: {:?}", e)))?
            .ok_or(DepositVerificationError::StorageError("Deposit address info not found".to_string()))?;

        let watch_runes_deposit_request = WatchRunesDepositRequest {
            musig_id: musig_id.clone(),
            nonce,
            address: deposit_addr_info.address.ok_or(DepositVerificationError::StorageError("Address not found".to_string()))?,
            amount: deposit_addr_info.amount,
            btc_address: btc_address.to_string(),
            txid,
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

        self.storage.update_confirmation_status_by_address(btc_address.to_string(), DepositStatusInfo {
            status: DepositStatus::WaitingForConfirmation,
            verifiers_responses,
        }).await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error updating confirmation status: {:?}", e)))?;

        self.storage.set_txid(btc_address.to_string(), txid).await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error setting txid: {:?}", e)))?;

        Ok(())
    }

    pub async fn notify_runes_deposit(
        &self,
        verifier_id: u16,
        txid: Txid,
        verifier_response: DepositStatus,
    ) -> Result<(), DepositVerificationError> {
        let mut confirmation_status_info = self.storage.get_confirmation_status_by_txid(txid).await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error getting confirmation status: {:?}", e)))?
            .ok_or(DepositVerificationError::StorageError("Confirmation status not found".to_string()))?;

        confirmation_status_info.verifiers_responses.responses.insert(verifier_id, verifier_response);
        let all_verifiers_confirmed = confirmation_status_info.verifiers_responses.check_all_verifiers_confirmed();

        if all_verifiers_confirmed {
            confirmation_status_info.status = DepositStatus::Confirmed;
        }

        self.storage.update_confirmation_status_by_txid(txid, confirmation_status_info).await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error updating confirmation status: {:?}", e)))?;

        let btc_address = self.storage.get_address_by_txid(txid).await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error getting address by txid: {:?}", e)))?
            .ok_or(DepositVerificationError::StorageError("Address not found".to_string()))?;

        if all_verifiers_confirmed {
            self.flow_sender.send(BridgeRunesRequest {
                btc_address,
            }).await
                .map_err(|e| DepositVerificationError::FlowProcessorError(format!("Error sending bridge runes request: {:?}", e)))?;
        }

        Ok(())
    }

    pub async fn verify_spark_deposit(
        &self,
        spark_address: String,
    ) -> Result<(), DepositVerificationError> {
        let (musig_id, nonce, deposit_addr_info) = self.storage.get_row_by_address(spark_address.to_string()).await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error getting deposit address info: {:?}", e)))?
            .ok_or(DepositVerificationError::StorageError("Deposit address info not found".to_string()))?;

        let watch_spark_deposit_request = WatchSparkDepositRequest {
            musig_id: musig_id.clone(),
            nonce,
            address: spark_address.clone(),
            amount: deposit_addr_info.amount,
            btc_address: deposit_addr_info.address.ok_or(DepositVerificationError::StorageError("Address not found".to_string()))?,
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
        let status = if all_verifiers_confirmed { DepositStatus::Confirmed } else { DepositStatus::Failed };

        self.storage.update_confirmation_status_by_address(spark_address.to_string(), DepositStatusInfo {
            status,
            verifiers_responses,
        }).await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error updating confirmation status: {:?}", e)))?;

        if all_verifiers_confirmed {
            self.flow_sender.send(ExitSparkRequest {
                spark_address,
            }).await
                .map_err(|e| DepositVerificationError::FlowProcessorError(format!("Error sending bridge spark request: {:?}", e)))?;
        }

        Ok(())
    }

}