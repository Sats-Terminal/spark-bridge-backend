use crate::error::DepositVerificationError;
use crate::traits::DepositVerificationClientTrait;
use crate::types::*;
use futures::future::join_all;
use gateway_flow_processor::flow_sender::{FlowSender, TypedMessageSender};
use gateway_flow_processor::types::{BridgeRunesRequest, ExitSparkRequest};
use gateway_local_db_store::schemas::deposit_address::{
    DepositAddressStorage, DepositStatus, InnerAddress, VerifiersResponses,
};
use gateway_local_db_store::schemas::paying_utxo::PayingUtxoStorage;
use gateway_local_db_store::schemas::utxo_storage::{Utxo, UtxoStatus, UtxoStorage};
use gateway_local_db_store::storage::LocalDbStorage;
use persistent_storage::init::StorageHealthcheck;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::instrument;

#[derive(Clone, Debug)]
pub struct DepositVerificationAggregator {
    flow_sender: FlowSender,
    verifiers: HashMap<u16, Arc<dyn DepositVerificationClientTrait>>,
    storage: Arc<LocalDbStorage>,
}

impl DepositVerificationAggregator {
    pub fn new(
        flow_sender: FlowSender,
        verifiers: HashMap<u16, Arc<dyn DepositVerificationClientTrait>>,
        storage: Arc<LocalDbStorage>,
    ) -> Self {
        Self {
            flow_sender,
            verifiers,
            storage,
        }
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub async fn verify_runes_deposit(
        &self,
        request: VerifyRunesDepositRequest,
    ) -> Result<(), DepositVerificationError> {
        tracing::info!("Verifying runes deposit for address: {}", request.btc_address);

        self.storage
            .update_bridge_address_by_deposit_address(
                InnerAddress::BitcoinAddress(request.btc_address.clone()),
                InnerAddress::SparkAddress(request.bridge_address.clone()),
            )
            .await?;

        let deposit_addr_info = self
            .storage
            .get_row_by_deposit_address(InnerAddress::BitcoinAddress(request.btc_address.clone()))
            .await?
            .ok_or(DepositVerificationError::NotFound(
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
            .set_confirmation_status_by_deposit_address(
                InnerAddress::BitcoinAddress(request.btc_address.clone()),
                verifiers_responses,
            )
            .await?;

        let utxo = Utxo {
            out_point: request.out_point,
            btc_address: request.btc_address.clone(),
            rune_amount: deposit_addr_info.amount,
            rune_id: deposit_addr_info.musig_id.get_rune_id(),
            status: UtxoStatus::Pending,
            sats_fee_amount: 0,
        };
        self.storage.insert_utxo(utxo).await?;

        tracing::info!("Runes deposit verification completed for address: {}", request.btc_address);

        Ok(())
    }

    #[instrument(level = "debug", skip(self), ret)]
    pub async fn notify_runes_deposit(
        &self,
        request: NotifyRunesDepositRequest,
    ) -> Result<(), DepositVerificationError> {
        tracing::info!("Gathering confirmation status for out point: {}", request.out_point);

        self.storage
            .update_sats_fee_amount(request.out_point, request.sats_fee_amount)
            .await?;

        let utxo = self
            .storage
            .get_utxo(request.out_point)
            .await?
            .ok_or(DepositVerificationError::NotFound("Address not found".to_string()))?;

        let btc_address = utxo.btc_address;

        self.storage
            .update_confirmation_status_by_deposit_address(
                InnerAddress::BitcoinAddress(btc_address.clone()),
                request.verifier_id,
                request.status,
            )
            .await?;

        let confirmation_status_info = self
            .storage
            .get_row_by_deposit_address(InnerAddress::BitcoinAddress(btc_address.clone()))
            .await?
            .ok_or(DepositVerificationError::NotFound(
                "Confirmation status not found".to_string(),
            ))?
            .confirmation_status;

        let all_verifiers_confirmed = confirmation_status_info.check_all_verifiers_confirmed();

        if all_verifiers_confirmed {
            self.storage
                .update_status(request.out_point, UtxoStatus::Confirmed)
                .await?;

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

        tracing::info!("Runes deposit verification completed for address: {}", btc_address);

        Ok(())
    }

    #[instrument(level = "debug", skip(self), ret)]
    pub async fn verify_spark_deposit(
        &self,
        request: VerifySparkDepositRequest,
    ) -> Result<(), DepositVerificationError> {
        tracing::info!("Verifying spark deposit for address: {}", request.spark_address);
        self.storage
            .update_bridge_address_by_deposit_address(
                InnerAddress::SparkAddress(request.spark_address.clone()),
                InnerAddress::BitcoinAddress(request.exit_address.clone()),
            )
            .await?;
        self.storage.insert_paying_utxo(request.paying_input).await?;

        let deposit_addr_info = self
            .storage
            .get_row_by_deposit_address(InnerAddress::SparkAddress(request.spark_address.clone()))
            .await?
            .ok_or(DepositVerificationError::NotFound(
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
            .set_confirmation_status_by_deposit_address(
                InnerAddress::SparkAddress(request.spark_address.clone()),
                verifiers_responses,
            )
            .await?;

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

        tracing::info!("Spark deposit verification completed for address: {}", request.spark_address);

        Ok(())
    }

    #[instrument(level = "trace", skip(self))]
    pub async fn healthcheck(&self) -> Result<(), DepositVerificationError> {
        self.storage.postgres_repo.healthcheck().await?;
        Self::check_set_of_verifiers(&self.verifiers).await?;
        Ok(())
    }

    #[instrument(level = "trace", skip(state), ret)]
    async fn check_set_of_verifiers(
        state: &HashMap<u16, Arc<dyn DepositVerificationClientTrait>>,
    ) -> Result<(), DepositVerificationError> {
        let mut join_set = JoinSet::new();
        for (v_id, v_client) in state.iter() {
            join_set.spawn({
                let (v_id, v_client) = (*v_id, v_client.clone());
                async move {
                    v_client
                        .healthcheck()
                        .await
                        .map_err(|e| DepositVerificationError::FailedToCheckStatusOfVerifier {
                            msg: e.to_string(),
                            id: v_id,
                        })
                }
            });
        }
        let _r = join_set.join_all().await.into_iter().collect::<Result<Vec<_>, _>>()?;
        Ok(())
    }
}
