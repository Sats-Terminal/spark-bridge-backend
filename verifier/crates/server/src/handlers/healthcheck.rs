use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use global_utils::common_resp::Empty;
use persistent_storage::init::StorageHealthcheck;
use tracing::trace;

#[tracing::instrument(skip_all, err)]
pub async fn handle(State(state): State<AppState>) -> Result<Json<Empty>, VerifierError> {
    trace!("Performing healthcheck for verifier...");
    state
        .storage
        .healthcheck()
        .await
        .map_err(|e| VerifierError::HealthcheckError(format!("Have a database error: [{e}]")))?;
    state
        .gateway_client
        .healthcheck()
        .await
        .map_err(|e| VerifierError::HealthcheckError(format!("Have a gateway error: [{e}]")))?;
    state
        .spark_balance_checker_client
        .healthcheck()
        .await
        .map_err(|e| VerifierError::HealthcheckError(format!("Have a spark balance checker error: [{e}]")))?;
    state
        .frost_signer
        .healthcheck()
        .map_err(|e| VerifierError::HealthcheckError(format!("Have a frost signer error: [{e}]")))?;
    Ok(Json(Empty {}))
}
