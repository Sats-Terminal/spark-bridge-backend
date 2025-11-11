use crate::error::DepositVerificationError;
use crate::traits::DepositVerificationClientTrait;
use crate::types::*;
use bitcoin::Network;
use bitcoin::secp256k1::PublicKey;
use frost::traits::AggregatorMusigIdStorage;
use frost::types::AggregatorDkgState;
use frost::types::AggregatorMusigIdData;
use futures::future::join_all;
use gateway_flow_processor::flow_sender::{FlowSender, TypedMessageSender};
use gateway_flow_processor::rune_metadata_client::{RuneMetadata, RuneMetadataClient};
use gateway_flow_processor::types::{BridgeRunesRequest, ExitSparkRequest};
use gateway_local_db_store::schemas::deposit_address::{
    DepositActivity, DepositAddressStorage, DepositStatus, InnerAddress, VerifiersResponses,
};
use gateway_local_db_store::schemas::paying_utxo::PayingUtxoStorage;
use gateway_local_db_store::schemas::rune_metadata::{RuneMetadataStorage, StoredRuneMetadata};
use gateway_local_db_store::schemas::utxo_storage::{Utxo, UtxoStatus, UtxoStorage};
use gateway_local_db_store::storage::LocalDbStorage;
use gateway_spark_service::utils::{RuneTokenConfig, WRunesMetadata, create_wrunes_metadata};
use global_utils::conversion::convert_network_to_spark_network;
use persistent_storage::init::StorageHealthcheck;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{instrument, warn};

#[derive(Clone, Debug)]
pub struct DepositVerificationAggregator {
    flow_sender: FlowSender,
    verifiers: HashMap<u16, Arc<dyn DepositVerificationClientTrait>>,
    storage: Arc<LocalDbStorage>,
    network: Network,
    rune_metadata_client: Option<RuneMetadataClient>,
}

impl DepositVerificationAggregator {
    pub fn new(
        flow_sender: FlowSender,
        verifiers: HashMap<u16, Arc<dyn DepositVerificationClientTrait>>,
        storage: Arc<LocalDbStorage>,
        network: Network,
    ) -> Self {
        let rune_metadata_client = match RuneMetadataClient::from_env() {
            Ok(client) => client,
            Err(err) => {
                warn!("Failed to initialize rune metadata client: {}", err);
                None
            }
        };
        Self {
            flow_sender,
            verifiers,
            storage,
            network,
            rune_metadata_client,
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

        tracing::info!(
            "Runes deposit verification sent for address: {}",
            request.btc_address.to_string()
        );

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

        let issuer_musig_id = self
            .storage
            .get_issuer_musig_id(deposit_addr_info.musig_id.get_rune_id())
            .await?
            .ok_or(DepositVerificationError::NotFound(
                "Issuer musig id not found".to_string(),
            ))?;
        let dkg_state = self.storage.get_musig_id_data(&issuer_musig_id).await?;

        let token_identifier = match dkg_state {
            Some(AggregatorMusigIdData {
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

                let rune_id = deposit_addr_info.musig_id.get_rune_id();
                let wrunes_metadata = match self.storage.get_rune_metadata(&rune_id).await? {
                    Some(entry) => match serde_json::from_value::<WRunesMetadata>(entry.wrune_metadata.clone()) {
                        Ok(metadata) => metadata,
                        Err(err) => {
                            tracing::warn!(
                                "Failed to decode cached wRune metadata for {}: {}. Recomputing.",
                                rune_id,
                                err
                            );
                            self.rebuild_wrune_metadata(&rune_id, musig_public_key).await?
                        }
                    },
                    None => self.rebuild_wrune_metadata(&rune_id, musig_public_key).await?,
                };
                wrunes_metadata.token_identifier
            }
            _ => {
                return Err(DepositVerificationError::NotFound(
                    "Token identifier not found".to_string(),
                ));
            }
        };

        tracing::debug!("Token identifier: {:?}", token_identifier.encode_bech32m(self.network));

        let watch_spark_deposit_request = WatchSparkDepositRequest {
            musig_id: deposit_addr_info.musig_id.clone(),
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

    #[instrument(level = "trace", skip(self))]
    pub async fn healthcheck(&self) -> Result<(), DepositVerificationError> {
        self.storage.postgres_repo.healthcheck().await?;
        Self::check_set_of_verifiers(&self.verifiers).await?;
        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub async fn list_user_activity(
        &self,
        user_public_key: bitcoin::secp256k1::PublicKey,
    ) -> Result<Vec<DepositActivity>, DepositVerificationError> {
        let activity = self
            .storage
            .list_deposit_activity_by_public_key(user_public_key)
            .await?;
        Ok(activity)
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub async fn get_activity_by_txid(&self, txid: &str) -> Result<Option<DepositActivity>, DepositVerificationError> {
        let activity = self.storage.get_deposit_activity_by_txid(txid).await?;
        Ok(activity)
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub async fn delete_pending_bridge_by_address(
        &self,
        btc_address: bitcoin::Address,
    ) -> Result<(), DepositVerificationError> {
        let removed = self
            .storage
            .delete_pending_deposit(InnerAddress::BitcoinAddress(btc_address))
            .await?;
        if removed {
            Ok(())
        } else {
            Err(DepositVerificationError::InvalidRequest(
                "Pending bridge request not found or already confirmed".to_string(),
            ))
        }
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub async fn list_wrune_metadata(&self) -> Result<Vec<StoredRuneMetadata>, DepositVerificationError> {
        let metadata = self.storage.list_rune_metadata().await?;
        Ok(metadata)
    }

    async fn rebuild_wrune_metadata(
        &self,
        rune_id: &str,
        musig_public_key: PublicKey,
    ) -> Result<WRunesMetadata, DepositVerificationError> {
        let rune_metadata = fetch_rune_metadata(&self.rune_metadata_client, rune_id).await;
        let rune_token_config = build_rune_token_config(rune_id, rune_metadata.as_ref());
        let wrunes_metadata =
            create_wrunes_metadata(&rune_token_config, musig_public_key, self.network).map_err(|e| {
                DepositVerificationError::InvalidDataError(format!(
                    "Failed to create wrunes metadata for {}: {}",
                    rune_id, e
                ))
            })?;

        self.persist_wrune_metadata(rune_id, rune_metadata.as_ref(), &wrunes_metadata, &musig_public_key)
            .await?;
        Ok(wrunes_metadata)
    }

    async fn persist_wrune_metadata(
        &self,
        rune_id: &str,
        rune_metadata: Option<&RuneMetadata>,
        wrune_metadata: &WRunesMetadata,
        musig_public_key: &PublicKey,
    ) -> Result<(), DepositVerificationError> {
        let rune_metadata_value = match rune_metadata {
            Some(metadata) => Some(serde_json::to_value(metadata).map_err(|err| {
                DepositVerificationError::InvalidDataError(format!(
                    "Failed to serialize rune metadata for {}: {}",
                    rune_id, err
                ))
            })?),
            None => None,
        };
        let wrune_metadata_value = serde_json::to_value(wrune_metadata).map_err(|err| {
            DepositVerificationError::InvalidDataError(format!(
                "Failed to serialize wRune metadata for {}: {}",
                rune_id, err
            ))
        })?;

        self.storage
            .upsert_rune_metadata(
                rune_id.to_string(),
                rune_metadata_value,
                wrune_metadata_value,
                musig_public_key.to_string(),
                self.network.to_string(),
                format!("{:?}", convert_network_to_spark_network(self.network)),
            )
            .await?;

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

async fn fetch_rune_metadata(client: &Option<RuneMetadataClient>, rune_id: &str) -> Option<RuneMetadata> {
    match client {
        Some(client) => match client.get_metadata(rune_id).await {
            Ok(metadata) => Some(metadata),
            Err(err) => {
                warn!("Failed to fetch rune metadata for {}: {}", rune_id, err);
                None
            }
        },
        None => None,
    }
}

fn build_rune_token_config(rune_id: &str, metadata: Option<&RuneMetadata>) -> RuneTokenConfig {
    RuneTokenConfig {
        rune_id: rune_id.to_string(),
        rune_name: metadata.map(|m| m.name.clone()),
        divisibility: metadata.map(|m| m.divisibility),
        max_supply: metadata.and_then(|m| m.max_supply),
        icon_url: metadata.and_then(|m| m.icon_url.clone()),
    }
}
