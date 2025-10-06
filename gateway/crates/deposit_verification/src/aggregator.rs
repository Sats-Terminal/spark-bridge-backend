use crate::error::DepositVerificationError;
use crate::traits::VerificationClient;
use crate::types::*;
use crate::types::{NotifyRunesDepositRequest, VerifyRunesDepositRequest, VerifySparkDepositRequest};
use bitcoin::Network;
use bitcoin::secp256k1::PublicKey;
use frost::traits::AggregatorDkgShareStorage;
use frost::types::{AggregatorDkgShareData, AggregatorDkgState};
use futures::future::join_all;
use gateway_flow_processor::flow_sender::{FlowSender, TypedMessageSender};
use gateway_flow_processor::types::{BridgeRunesRequest, ExitSparkRequest};
use gateway_local_db_store::schemas::deposit_address::{
    DepositAddressStorage, DepositStatus, InnerAddress, VerifiersResponses,
};
use gateway_local_db_store::schemas::paying_utxo::PayingUtxoStorage;
use gateway_local_db_store::schemas::user_identifier::{UserIdentifierStorage, UserIds};
use gateway_local_db_store::schemas::utxo_storage::{Utxo, UtxoStatus, UtxoStorage};
use gateway_local_db_store::storage::LocalDbStorage;
use gateway_spark_service::utils::create_wrunes_metadata;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;

#[derive(Clone, Debug)]
pub struct DepositVerificationAggregator {
    flow_sender: FlowSender,
    verifiers: HashMap<u16, Arc<dyn VerificationClient>>,
    storage: Arc<LocalDbStorage>,
    network: Network,
}

impl DepositVerificationAggregator {
    pub fn new(
        flow_sender: FlowSender,
        verifiers: HashMap<u16, Arc<dyn VerificationClient>>,
        storage: Arc<LocalDbStorage>,
        network: Network,
    ) -> Self {
        Self {
            flow_sender,
            verifiers,
            storage,
            network,
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
        let user_ids = self
            .storage
            .get_row_by_dkg_id(deposit_addr_info.dkg_share_id)
            .await?
            .ok_or(DepositVerificationError::NotFound(
                "Deposit address info not found".to_string(),
            ))?;

        let watch_runes_deposit_request = WatchRunesDepositRequest {
            user_ids: UserIds {
                user_id: user_ids.user_id,
                dkg_share_id: user_ids.dkg_share_id,
                rune_id: user_ids.rune_id.clone(),
                is_issuer: false
            },
            nonce: deposit_addr_info.nonce,
            amount: deposit_addr_info.amount,
            btc_address: request.btc_address.clone(),
            bridge_address: request.bridge_address.clone(),
            out_point: request.out_point,
        };

        let mut futures = Vec::with_capacity(self.verifiers.len());
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
            rune_id: user_ids.rune_id,
            status: UtxoStatus::Pending,
            sats_fee_amount: 0,
        };
        self.storage.insert_utxo(utxo).await?;

        tracing::info!("Runes deposit verification sent to address: {}", request.btc_address);

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
        tracing::info!("Verifying spark deposit for spark address: {}", request.spark_address);
        self.storage
            .update_bridge_address_by_deposit_address(
                InnerAddress::SparkAddress(request.spark_address.clone()),
                InnerAddress::BitcoinAddress(request.paying_input.btc_exit_address.clone()),
            )
            .await?;
        self.storage.insert_paying_utxo(request.paying_input.clone()).await?;

        let deposit_addr_info = self
            .storage
            .get_row_by_deposit_address(InnerAddress::SparkAddress(request.spark_address.clone()))
            .await?
            .ok_or(DepositVerificationError::NotFound(
                "Deposit address info not found".to_string(),
            ))?;
        
        let user_ids = self
            .storage
            .get_row_by_dkg_id(deposit_addr_info.dkg_share_id)
            .await?
            .ok_or(DepositVerificationError::NotFound(
                "Deposit address info not found".to_string(),
            ))?;
        
        let issuer_ids = self.storage.get_issuer_ids(user_ids.rune_id.clone()).await?
            .ok_or(DepositVerificationError::NotFound(
                "Issuer ids not found".to_string(),
            ))?;

        let dkg_state = self.storage.get_dkg_share_agg_data(&issuer_ids.dkg_share_id).await?;

        let token_identifier = match dkg_state {
            Some(AggregatorDkgShareData {
                dkg_state: AggregatorDkgState::DkgFinalized { public_key_package },
            }) => {
                let musig_public_key_bytes = public_key_package.verifying_key().serialize().map_err(|e| {
                    DepositVerificationError::InvalidDataError(format!(
                        "Failed to serialize issuer musig public key: {}",
                        e
                    ))
                })?;
                let musig_public_key = PublicKey::from_slice(&musig_public_key_bytes).map_err(|e| {
                    DepositVerificationError::InvalidDataError(format!(
                        "Failed to deserialize issuer musig public key: {}",
                        e
                    ))
                })?;

                let wrunes_metadata = create_wrunes_metadata(
                    issuer_ids.rune_id.clone(),
                    musig_public_key,
                    self.network,
                )
                .map_err(|e| {
                    DepositVerificationError::InvalidDataError(format!("Failed to create wrunes metadata: {}", e))
                })?;
                wrunes_metadata.token_identifier
            }
            _ => {
                return Err(DepositVerificationError::NotFound(
                    "Token identifier not found".to_string(),
                ));
            }
        };

        tracing::debug!("Token identifier: {:?}", token_identifier.encode_bech32m(self.network));
        //todo: check finish

        let watch_spark_deposit_request = WatchSparkDepositRequest {
            user_ids: user_ids.clone(),
            nonce: deposit_addr_info.nonce,
            spark_address: request.spark_address.clone(),
            amount: deposit_addr_info.amount,
            exit_address: request.paying_input.btc_exit_address.clone(),
            token_identifier,
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
            tracing::info!("All verifiers confirmed for spark address: {}", request.spark_address);
            self.flow_sender
                .send(ExitSparkRequest {
                    spark_address: request.spark_address.clone(),
                })
                .await
                .map_err(|e| {
                    DepositVerificationError::FlowProcessorError(format!("Error sending bridge spark request: {:?}", e))
                })?;
        }

        tracing::info!(
            "Spark deposit verification completed for address: {}",
            request.spark_address
        );

        Ok(())
    }
}
