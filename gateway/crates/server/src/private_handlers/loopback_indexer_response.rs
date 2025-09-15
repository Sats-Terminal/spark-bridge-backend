use crate::error::PrivateApiError;
use crate::init::PrivateAppState;
use axum::Json;
use axum::extract::State;
use gateway_api::api::{Review, TxCheckCallbackResponse};
use serde::{Deserialize, Serialize};
use titan_client::Transaction;
use tracing::{info, instrument};

/// Handles Btc address issuing for replenishment
#[instrument(level = "info", skip(state, request), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<PrivateAppState>,
    //todo: change Transaction on another type generated from gateway or remove it
    Json(request): Json<TxCheckCallbackResponse>,
) -> Result<Json<()>, PrivateApiError> {
    _handle_inner(state, request)
        .await
        .map_err(|e| PrivateApiError::InvalidResponseType(format!("Failed to issue deposit address for bridging: {e}")))
}

#[instrument(level = "debug", skip(state, request), fields(request = ?request), ret)]
async fn _handle_inner(state: PrivateAppState, request: TxCheckCallbackResponse) -> anyhow::Result<Json<()>> {
    info!("Invoking handle inner with request: {:?}", request);
    //todo: spawn task to make spark transaction if all is ok, if not, save error
    state.btc_resp_checker.save_verifier_response(request).await?;
    Ok(Json(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Txid;
    use bitcoin::hashes::sha256d::Hash;
    use global_utils::api_result_request::ApiResponseOwned;
    use std::str::FromStr;
    use titan_client::{Transaction, TransactionStatus};

    #[test]
    fn tests() {
        let x: ApiResponseOwned<Transaction> = ApiResponseOwned::Ok {
            data: Transaction {
                txid: Txid::from_raw_hash(
                    Hash::from_str("f9d3ba0ce47bf605245fcb46340ef210e0da2c256034eda5a8abb86d1e7044c4").unwrap(),
                ),
                version: 20,
                lock_time: 0,
                input: vec![],
                output: vec![],
                status: TransactionStatus {
                    confirmed: true,
                    block_height: Some(100),
                    block_hash: None,
                },
                size: 0,
                weight: 0,
            },
        };
        println!("{}", serde_json::to_string_pretty(&x).unwrap());
    }
}
