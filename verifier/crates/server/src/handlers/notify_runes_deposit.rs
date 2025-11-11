use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::OutPoint;
use serde::{Deserialize, Serialize};
use spark_balance_checker_server::models::DepositStatus as SparkDepositStatus;
use spark_balance_checker_server::models::VerifyBalanceRequest;
use token_identifier::TokenIdentifier;
use uuid::Uuid;
use verifier_btc_indexer_client::client::WatchRunesDepositRequest as IndexerWatchRunesDepositRequest;
use verifier_config_parser::config::construct_hardcoded_callback_url;
use verifier_gateway_client::client::{
    GatewayDepositStatus, GatewayNotifyRunesDepositRequest, GatewayNotifySparkDepositRequest,
};
use verifier_local_db_store::schemas::deposit_address::{DepositAddressStorage, DepositStatus, FeePayment};
use verifier_spark_balance_checker_client::client::cast_deposit_status;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum BtcIndexerDepositStatus {
    Confirmed,
    Failed,
}

impl Into<DepositStatus> for BtcIndexerDepositStatus {
    fn into(self) -> DepositStatus {
        match self {
            BtcIndexerDepositStatus::Confirmed => DepositStatus::Confirmed,
            BtcIndexerDepositStatus::Failed => DepositStatus::Failed,
        }
    }
}

impl Into<GatewayDepositStatus> for BtcIndexerDepositStatus {
    fn into(self) -> GatewayDepositStatus {
        match self {
            BtcIndexerDepositStatus::Confirmed => GatewayDepositStatus::Confirmed,
            BtcIndexerDepositStatus::Failed => GatewayDepositStatus::Failed,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct IndexerNotifyRequest {
    pub outpoint: OutPoint,
    pub request_id: Uuid,
    pub deposit_status: BtcIndexerDepositStatus,
    pub sats_amount: Option<u64>,
    pub rune_id: Option<String>,
    pub rune_amount: Option<u64>,
    pub error_details: Option<String>,
}

pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<IndexerNotifyRequest>,
) -> Result<Json<()>, VerifierError> {
    tracing::info!("Got deposit notification for out point: {}", request.outpoint);

    let sats_amount = request
        .sats_amount
        .ok_or(VerifierError::Validation("Sats amount is required".to_string()))?;

    // Workaround to handle fee payments in existing code structure
    // If no rune amount is provided, assume it's a fee payment notification
    if request.rune_amount.is_none() {
        handle_fee_notification(request.clone(), &state).await?;
        tracing::info!("BTC payment notified for out point: {}", request.outpoint);
        return Ok(Json(()));
    }

    // Set confirmation status only for deposit addresses where fee payment was confirmed previously via branch above
    state
        .storage
        .set_confirmation_status_by_out_point(request.outpoint, request.deposit_status.clone().into(), None)
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set confirmation status: {}", e)))?;

    state
        .storage
        .set_sats_amount_by_out_point(request.outpoint, sats_amount)
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set sats amount: {}", e)))?;

    let gateway_request = GatewayNotifyRunesDepositRequest {
        verifier_id: state.server_config.frost_signer.identifier,
        request_id: request.request_id,
        outpoint: request.outpoint,
        sats_amount: sats_amount,
        status: request.deposit_status.clone().into(),
        error_details: request.error_details,
    };

    state
        .gateway_client
        .notify_runes_deposit(gateway_request)
        .await
        .map_err(|e| VerifierError::GatewayClient(format!("Failed to notify runes deposit: {}", e)))?;

    tracing::info!("Runes deposit notified for out point: {}", request.outpoint);

    Ok(Json(()))
}

async fn handle_fee_notification(request: IndexerNotifyRequest, state: &AppState) -> Result<(), VerifierError> {
    state
        .storage
        .set_fee_status_by_payment(
            FeePayment::Btc(request.outpoint),
            request.deposit_status.into(),
            request.error_details.clone(),
        )
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set fee status: {}", e)))?;

    let deposit_addr_info = state
        .storage
        .get_deposit_addr_info_by_confirmed_fee_payment(FeePayment::Btc(request.outpoint))
        .await
        .map_err(|err| VerifierError::Storage(err.to_string()))?;

    if deposit_addr_info.is_btc {
        let callback_url = construct_hardcoded_callback_url(&state.server_config.server);
        state
            .btc_indexer_client
            .watch_runes_deposit(IndexerWatchRunesDepositRequest {
                request_id: request.request_id,
                btc_address: deposit_addr_info.deposit_address.to_string(),
                outpoint: deposit_addr_info.outpoint.unwrap_or_default(),
                rune_id: Some(deposit_addr_info.token),
                rune_amount: Some(deposit_addr_info.deposit_amount),
                sats_amount: None,
                callback_url: callback_url.to_string(),
            })
            .await
            .map_err(|e| VerifierError::BtcIndexerClient(format!("Failed to watch runes deposit: {}", e)))?;
    } else {
        let response = state
            .spark_balance_checker_client
            .verify_balance(VerifyBalanceRequest {
                request_id: request.request_id,
                spark_address: deposit_addr_info.deposit_address.to_string(),
                token_identifier: TokenIdentifier::from_str(
                    &deposit_addr_info.token,
                    state.server_config.server.network,
                )
                .map_err(|err| VerifierError::Validation(err.to_string()))?,
                amount: deposit_addr_info.deposit_amount as u128,
            })
            .await
            .map_err(|e| VerifierError::SparkBalanceCheckerClient(format!("Failed to verify balance: {}", e)))?;

        state
            .storage
            .set_confirmation_status_by_deposit_address(
                deposit_addr_info.deposit_address.clone(),
                cast_deposit_status(&response.deposit_status),
                response.error_details,
            )
            .await
            .map_err(|e| VerifierError::Storage(format!("Failed to update confirmation status: {}", e)))?;

        state
            .gateway_client
            .notify_spark_deposit(GatewayNotifySparkDepositRequest {
                verifier_id: state.server_config.frost_signer.identifier,
                request_id: request.request_id,
                spark_address: deposit_addr_info.deposit_address.to_string(),
                status: match response.deposit_status {
                    SparkDepositStatus::Confirmed => GatewayDepositStatus::Confirmed,
                    SparkDepositStatus::Failed => GatewayDepositStatus::Failed,
                },
            })
            .await
            .map_err(|e| VerifierError::GatewayClient(format!("Failed to notify spark deposit: {}", e)))?;

        tracing::info!("Spark deposit notified for address: {}", request.outpoint);
    }

    tracing::warn!("handled btc notification");
    Ok(())
}
