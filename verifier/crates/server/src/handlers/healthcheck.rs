use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use global_utils::common_resp::Empty;
use persistent_storage::init::StorageHealthcheck;

#[tracing::instrument(skip_all, err)]
pub async fn handle(State(state): State<AppState>) -> Result<Json<Empty>, VerifierError> {
    state
        .storage
        .healthcheck()
        .await
        .map_err(|e| VerifierError::Healthcheck(format!("Have a database error: [{e}]")))?;
    state
        .spark_balance_checker_client
        .healthcheck()
        .await
        .map_err(|e| VerifierError::Healthcheck(format!("Have a spark balance checker error: [{e}]")))?;
    Ok(Json(Empty {}))
}
