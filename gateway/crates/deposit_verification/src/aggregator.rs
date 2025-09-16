use gateway_flow_processor::flow_sender::{FlowSender, TypedMessageSender};
use crate::traits::VerificationClient;
use std::sync::Arc;
use crate::types::*;
use crate::error::DepositVerificationError;
use gateway_local_db_store::schemas::deposit_address::{DepositAddressStorage, DepositStatusInfo, DepositStatus};
use bitcoin::{Address, Txid};
use futures::future::join_all;
use std::collections::HashMap;
use gateway_config_parser::config::VerifierConfig;
use gateway_flow_processor::types::BridgeRunesRequest;

#[derive(Clone, Debug)]
pub struct DepositVerificationAggregator {
    verifier_configs: Vec<VerifierConfig>,
    flow_sender: FlowSender,
    verifiers: Vec<Arc<dyn VerificationClient>>,
    storage: Arc<dyn DepositAddressStorage>,
}

impl DepositVerificationAggregator {
    pub fn new(
        verifier_configs: Vec<VerifierConfig>,
        flow_sender: FlowSender,
        verifiers: Vec<Arc<dyn VerificationClient>>,
        storage: Arc<dyn DepositAddressStorage>,
    ) -> Self {
        Self { verifier_configs, flow_sender, verifiers, storage }
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

        for verifier in self.verifiers.iter() {
            let watch_runes_deposit_request_clone = watch_runes_deposit_request.clone();
            let join_handle = async move { verifier.watch_runes_deposit(watch_runes_deposit_request_clone).await };
            futures.push(join_handle);
        }

        let _ = join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<WatchRunesDepositResponse>, DepositVerificationError>>()?;

        self.storage.update_confirmation_status_by_address(btc_address.to_string(), DepositStatusInfo {
            txid: None,
            status: DepositStatus::WaitingForConfirmation,
            verifiers_responses: None,
        }).await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error updating confirmation status: {:?}", e)))?;

        Ok(())
    }

    pub async fn notify_runes_deposit(
        &self,
        verifier_id: u16,
        btc_address: Address,
    ) -> Result<(), DepositVerificationError> {
        let mut confirmation_status_info = self.storage.get_confirmation_status_by_address(btc_address.to_string()).await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error getting confirmation status: {:?}", e)))?
            .ok_or(DepositVerificationError::StorageError("Confirmation status not found".to_string()))?;

        let mut confirmation_status = confirmation_status_info.verifiers_responses.unwrap_or_default();
        confirmation_status.insert(verifier_id, true);
        let all_verifiers_confirmed = self.check_all_verifiers_confirmed(&confirmation_status);
        confirmation_status_info.verifiers_responses = Some(confirmation_status);

        if all_verifiers_confirmed {
            confirmation_status_info.status = DepositStatus::Confirmed;
        }

        self.storage.update_confirmation_status_by_address(btc_address.to_string(), confirmation_status_info).await
            .map_err(|e| DepositVerificationError::StorageError(format!("Error updating confirmation status: {:?}", e)))?;

        if all_verifiers_confirmed {
            self.flow_sender.send(BridgeRunesRequest {
                address: btc_address,
            }).await
                .map_err(|e| DepositVerificationError::FlowProcessorError(format!("Error sending bridge runes request: {:?}", e)))?;
        }

        Ok(())
    }

    pub fn check_all_verifiers_confirmed(&self, confirmation_status: &HashMap<u16, bool>) -> bool {
        for verifier_config in self.verifier_configs.iter() {
            if !confirmation_status.contains_key(&verifier_config.id) {
                return false;
            }
        }
        true
    }

}