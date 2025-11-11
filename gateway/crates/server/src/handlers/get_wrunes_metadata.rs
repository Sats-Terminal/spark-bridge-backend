use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use gateway_local_db_store::schemas::rune_metadata::StoredRuneMetadata;
use tracing::instrument;

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(State(state): State<AppState>) -> Result<Json<Vec<StoredRuneMetadata>>, GatewayError> {
    let metadata = state
        .deposit_verification_aggregator
        .list_wrune_metadata()
        .await
        .map_err(|err| GatewayError::DepositVerificationError(err.to_string()))?;

    Ok(Json(metadata))
}
