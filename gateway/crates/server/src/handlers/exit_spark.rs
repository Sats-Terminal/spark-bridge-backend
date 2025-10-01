use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::{Network, Txid, secp256k1::schnorr::Signature};
use gateway_deposit_verification::types::VerifySparkDepositRequest;
use gateway_rune_transfer::transfer::PayingTransferInput;
use global_utils::conversion::decode_address;
use serde::Deserialize;
use std::str::FromStr;
use tracing;

#[derive(Deserialize)]
pub struct UserPayingTransferInput {
    pub txid: String,
    pub vout: u32,
    pub address: String,
    pub sats_amount: u64,
    pub none_anyone_can_pay_signature: Signature,
}

impl UserPayingTransferInput {
    fn try_into(self, network: Network) -> Result<PayingTransferInput, GatewayError> {
        Ok(PayingTransferInput {
            txid: Txid::from_str(&self.txid)
                .map_err(|e| GatewayError::InvalidData(format!("Failed to parse txid: {e}")))?,
            vout: self.vout,
            address: decode_address(&self.address, network)
                .map_err(|e| GatewayError::InvalidData(format!("Failed to parse address: {e}")))?,
            sats_amount: self.sats_amount,
            none_anyone_can_pay_signature: Signature::from_slice(&self.none_anyone_can_pay_signature.serialize())
                .map_err(|e| {
                    GatewayError::InvalidData(format!("Failed to parse none anyone can pay signature: {e}"))
                })?,
        })
    }
}

#[derive(Deserialize)]
pub struct ExitSparkRequest {
    pub spark_address: String,
    pub exit_address: String,
    pub paying_input: UserPayingTransferInput,
}

pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<ExitSparkRequest>,
) -> Result<Json<()>, GatewayError> {
    tracing::debug!("Exit spark request for spark address: {:?}", request.spark_address);
    let verify_spark_deposit_request = VerifySparkDepositRequest {
        spark_address: request.spark_address,
        exit_address: decode_address(&request.exit_address, state.network)
            .map_err(|e| GatewayError::InvalidData(format!("Failed to parse exit address: {e}")))?,
        paying_input: request.paying_input.try_into(state.network)?,
    };

    state
        .deposit_verification_aggregator
        .verify_spark_deposit(verify_spark_deposit_request)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(format!("Failed to verify spark deposit: {}", e)))?;

    tracing::debug!("Exit spark request verified");
    Ok(Json(()))
}
