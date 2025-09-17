use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use btc_indexer_api::api::BtcIndexerCallbackResponse;
use tracing::instrument;
use verifier_local_db_store::schemas::deposit_address::DepositStatus;
use global_utils::api_result_request::ApiResponseOwned;
use verifier_local_db_store::schemas::deposit_address::DepositAddressStorage;
use verifier_gateway_client::client::NotifyRunesDepositRequest;

#[instrument(level = "debug", skip_all, ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<BtcIndexerCallbackResponse>,
) -> Result<Json<()>, VerifierError> {
    // TODO: This request should spawn task and immediately return Json(())
    match request {
        ApiResponseOwned::Ok { data: transaction } => {
            let txid = transaction.txid;
            let verifier_response = DepositStatus::Confirmed;

            state.storage.set_confirmation_status_by_txid(txid, verifier_response.clone())
                .await
                .map_err(|e| VerifierError::StorageError(format!("Failed to update confirmation status: {}", e)))?;

            state.gateway_client.notify_runes_deposit(NotifyRunesDepositRequest {
                verifier_id: state.server_config.frost_signer.identifier,
                txid,
                verifier_response,
            }).await.map_err(|e| VerifierError::GatewayClientError(format!("Failed to notify runes deposit: {}", e)))?;

            Ok(Json(()))
        }
        ApiResponseOwned::Err { code: _, message } => {
            // TODO: I should set the confirmation status to failed, but I do not get txid.
            Err(VerifierError::BtcIndexerClientError(format!("Failed to notify runes deposit: {}", message)))
        }
    }
}
