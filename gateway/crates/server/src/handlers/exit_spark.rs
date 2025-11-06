use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::{Network, Txid, secp256k1::schnorr::Signature};
use gateway_deposit_verification::types::{FeePayment, VerifySparkDepositRequest};
use gateway_rune_transfer::transfer::PayingTransferInput;
use global_utils::conversion::decode_address;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::instrument;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct UserPayingTransferInput {
    pub txid: String,
    pub vout: u32,
    pub btc_exit_address: String,
    pub sats_amount: u64,
    pub none_anyone_can_pay_signature: Signature,
}

impl UserPayingTransferInput {
    fn try_into(self, network: Network) -> Result<PayingTransferInput, GatewayError> {
        Ok(PayingTransferInput {
            txid: Txid::from_str(&self.txid)
                .map_err(|e| GatewayError::InvalidData(format!("Failed to parse txid: {e}")))?,
            vout: self.vout,
            btc_exit_address: decode_address(&self.btc_exit_address, network)
                .map_err(|e| GatewayError::InvalidData(format!("Failed to parse address: {e}")))?,
            sats_amount: self.sats_amount,
            none_anyone_can_pay_signature: self.none_anyone_can_pay_signature,
        })
    }
}

#[derive(Deserialize, Debug)]
pub struct ExitSparkRequest {
    pub spark_address: String,
    pub paying_input: UserPayingTransferInput,
    pub fee_payment: FeePayment,
}

#[derive(Serialize, Debug)]
pub struct ExitSparkResponse {
    pub request_id: Uuid,
}

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<ExitSparkRequest>,
) -> Result<Json<ExitSparkResponse>, GatewayError> {
    let request_spark_address = request.spark_address.clone();
    let request_id = Uuid::new_v4();
    tracing::info!(
        "Handling exit spark request with spark address: {:?}",
        request_spark_address
    );

    let verify_spark_deposit_request = VerifySparkDepositRequest {
        request_id,
        spark_address: request.spark_address,
        paying_input: request.paying_input.try_into(state.network)?,
        fee_payment: request.fee_payment,
    };

    state
        .deposit_verification_aggregator
        .verify_spark_deposit(verify_spark_deposit_request)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(format!("Failed to verify spark deposit: {}", e)))?;

    tracing::info!(
        "Exit spark request handled request with spark address: {:?}",
        request_spark_address
    );

    Ok(Json(ExitSparkResponse { request_id }))
}
